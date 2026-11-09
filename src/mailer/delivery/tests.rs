use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use super::*;
use crate::mailer::{build_message_with, SmtpTls};

fn email() -> Email {
    Email::builder().to("ann@example.com").cc("cc@example.com").subject("Your report; 100% \"done\"").text("Hi\n.dot line").html("<p>Hi</p>").reply_to("help@example.com").build().unwrap()
}

#[test]
fn attachments_make_a_multipart_mixed_message() {
    let pdf = MailAttachment::new("Q3 \"report\"\r\nBcc: x.pdf", "application/pdf", vec![0u8, 1, 2, 255]);
    let msg = build_message_with("from@example.com", &email(), &[pdf, MailAttachment::new("a.txt", "text/plain", "x".repeat(100))]);
    assert!(msg.contains("Content-Type: multipart/mixed; boundary=\"----=_Part_rws_mixed\"\r\n\r\n------=_Part_rws_mixed\r\nContent-Type: multipart/alternative"), "{msg}");
    // The name can't break out of its quotes or start a header.
    assert!(msg.contains("Content-Disposition: attachment; filename=\"Q3 _report_  Bcc: x.pdf\"\r\n"), "{msg}");
    assert!(!msg.contains("\r\nBcc:"));
    assert!(msg.contains("Content-Transfer-Encoding: base64\r\n\r\nAAEC/w=="), "{msg}");
    // Base64 lines are at most 76 characters.
    assert!(msg.lines().all(|l| l.len() <= 998));
    let b64_lines: Vec<&str> = msg.split("filename=\"a.txt\"\r\nContent-Transfer-Encoding: base64\r\n\r\n").nth(1).unwrap().lines().take(2).collect();
    assert_eq!(76, b64_lines[0].len());
    assert!(msg.ends_with("\r\n------=_Part_rws_mixed--\r\n"));
    // Without attachments the message is as before.
    assert!(!build_message_with("f@example.com", &email(), &[]).contains("multipart/mixed"));
}

#[test]
fn payload_round_trips() {
    let mail = Mail::new(email()).attach(MailAttachment::new("r;e=p%ort.pdf", "application/pdf", vec![9u8; 300]));
    let payload = mail.to_payload();
    assert!(!payload.contains("\nHi\n"), "newlines in values are encoded: {payload}");
    assert_eq!(mail, Mail::from_payload(&payload).unwrap());
    assert!(Mail::from_payload("").is_err(), "no recipient");
    assert!(Mail::from_payload("to=a@b\nweird=1").is_err());
    assert!(Mail::from_payload("to=a@b\nattachment=x;y").is_err());
    assert!(Mail::from_payload("to=a@b\nattachment=x;y;***").is_err());
    assert!(Mail::from_payload("to=%ZZ").is_err());
    assert!(Mail::from_payload("no equals sign").is_err());
}

#[test]
fn test_mailbox_keeps_what_it_gets() {
    let mailbox = TestMailbox::new();
    let clone = mailbox.clone();
    assert!(mailbox.last().is_none());
    clone.deliver(&Mail::new(email())).unwrap();
    let other = Email::builder().to("bob@example.com").subject("B").text("b").build().unwrap();
    clone.deliver(&other.into()).unwrap();
    assert_eq!(2, mailbox.count());
    assert_eq!("B", mailbox.last().unwrap().email.subject);
    assert_eq!("Your report; 100% \"done\"", mailbox.last_to("CC@example.com").unwrap().email.subject);
    assert!(mailbox.last_to("nobody@example.com").is_none());
    assert_eq!(2, mailbox.sent().len());
    mailbox.clear();
    assert_eq!(0, clone.count());
}

/// One SMTP session; answers everything and returns the DATA it got.
fn mock_smtp() -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut r = BufReader::new(stream.try_clone().unwrap());
        let mut w = stream;
        write!(w, "220 mock\r\n").unwrap();
        let mut data = String::new();
        let mut in_data = false;
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if in_data {
                if line == ".\r\n" {
                    in_data = false;
                    write!(w, "250 queued\r\n").unwrap();
                } else {
                    data.push_str(&line);
                }
                continue;
            }
            match line.get(..4).unwrap_or("") {
                "DATA" => {
                    in_data = true;
                    write!(w, "354 go\r\n").unwrap();
                }
                "QUIT" => {
                    write!(w, "221 bye\r\n").unwrap();
                    break;
                }
                _ => write!(w, "250 ok\r\n").unwrap(),
            }
        }
        data
    });
    (port, handle)
}

#[test]
fn mailer_delivers_attachments_over_smtp() {
    let (port, server) = mock_smtp();
    let mailer = Mailer { host: "127.0.0.1".into(), port, user: None, password: None, from: "app@example.com".into(), tls: SmtpTls::None, timeout_ms: 5000 };
    let mail = Mail::new(email()).attach(MailAttachment::new("hi.txt", "text/plain", "hello"));
    Deliver::deliver(&mailer, &mail).unwrap();
    let data = server.join().unwrap();
    assert!(data.contains("multipart/mixed") && data.contains("filename=\"hi.txt\"") && data.contains("aGVsbG8="), "{data}");
}

#[cfg(feature = "jobs")]
#[test]
fn deliver_later_sends_on_a_worker() {
    let queue = crate::jobs::JobQueue::new(1);
    let mailbox = TestMailbox::new();
    Mail::new(email()).deliver_later(&queue, Arc::new(mailbox.clone()));
    queue.join();
    assert_eq!(1, mailbox.count());
}

#[cfg(all(feature = "jobs", feature = "model-sqlite"))]
#[tokio::test]
async fn persistent_delivery_goes_through_the_queue() {
    let queue = crate::jobs::PersistentJobQueue::new(crate::model::DbPool::memory().await.unwrap()).await.unwrap();
    let mailbox = TestMailbox::new();
    register_delivery(&queue, Arc::new(mailbox.clone()));
    let mail = Mail::new(email()).attach(MailAttachment::new("a.bin", "application/octet-stream", vec![1u8, 2, 3]));
    mail.enqueue(&queue, crate::jobs::EnqueueOptions::new().queue("mailers")).await.unwrap();
    assert_eq!(0, mailbox.count());
    assert!(queue.tick().await.unwrap());
    assert_eq!(Some(mail), mailbox.last());
    // A payload that isn't a mail fails like any job.
    queue.enqueue_with(MAIL_JOB_TYPE, "garbage", crate::jobs::EnqueueOptions::new().max_retries(0)).await.unwrap();
    assert!(queue.tick().await.unwrap());
    assert_eq!(1, queue.failed_jobs(5).await.unwrap().len());
}

#[cfg(feature = "tera")]
#[test]
fn emails_render_from_templates() {
    let engine = crate::template::TeraEngine::from_raw(&[
        ("welcome.txt", "Hi {{ name }}"),
        ("welcome.html", "<p>Hi {{ name }}</p>"),
        ("receipt.txt", "Paid {{ amount }}"),
    ])
    .unwrap();
    let mut ctx = crate::template::Context::new();
    ctx.insert("name", "<Ann>");
    ctx.insert("amount", "5");
    let welcome = Email::builder().to("a@b.c").subject("Welcome").render_with(&engine, "welcome", &ctx).unwrap().build().unwrap();
    assert_eq!(Some("Hi <Ann>".to_string()), welcome.text);
    assert_eq!(Some("<p>Hi &lt;Ann&gt;</p>".to_string()), welcome.html);
    let receipt = Email::builder().to("a@b.c").subject("R").render_with(&engine, "receipt", &ctx).unwrap().build().unwrap();
    assert_eq!((Some("Paid 5".to_string()), None), (receipt.text, receipt.html));
    assert!(Email::builder().render_with(&engine, "missing", &ctx).is_err());
}

#[test]
fn previews_show_each_email() {
    use crate::app::App;
    use crate::test_client::TestClient;
    let previews = MailPreviews::new("/_mail/")
        .add("welcome", || Ok(Mail::new(email()).attach(MailAttachment::new("a.pdf", "application/pdf", vec![1u8]))))
        .add("text_only", || Ok(Email::builder().to("t@example.com").subject("T").text("<plain>").build()?.into()))
        .add("broken", || Err(MailerError::Build("no sample data".into())));
    let app = App::with_state(()).get("/", |_, _, _, _| crate::response::Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "app")).wrap(previews);
    let client = TestClient::new(app);

    let index = client.get("/_mail").send();
    assert!(index.body_text().contains(r#"<a href="/_mail/welcome">welcome</a>"#), "{}", index.body_text());
    let page = client.get("/_mail/welcome").send().body_text().to_string();
    assert!(page.contains("Your report; 100% &quot;done&quot;") && page.contains("a.pdf (application/pdf, 1 bytes)") && page.contains(r#"<iframe sandbox src="/_mail/welcome.html""#), "{page}");
    assert_eq!("<p>Hi</p>", client.get("/_mail/welcome.html").send().body_text());
    assert_eq!("Hi\n.dot line", client.get("/_mail/welcome.txt").send().body_text());
    assert!(client.get("/_mail/text_only").send().body_text().contains("<pre>&lt;plain&gt;</pre>"));
    assert_eq!(404, client.get("/_mail/text_only.html").send().status());
    assert_eq!(404, client.get("/_mail/nope").send().status());
    assert_eq!(500, client.get("/_mail/broken").send().status());
    // Everything else reaches the app.
    assert_eq!("app", client.get("/").send().body_text());
}
