#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use crate::mailer::{build_message, dot_stuff, Email, MailerError, Mailer, SmtpTls};

    // ── Mock SMTP server ──────────────────────────────────────────────────────

    /// Spawns a minimal SMTP server on an ephemeral port.
    ///
    /// Handles exactly one connection. Records every line sent by the client
    /// into `recv`. `with_auth`: if true, advertises `AUTH PLAIN` in EHLO and
    /// expects an `AUTH PLAIN …` command before `MAIL FROM`.
    fn start_mock_smtp(recv: Arc<Mutex<Vec<String>>>, with_auth: bool) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut w = stream;

            let record = |recv: &Arc<Mutex<Vec<String>>>, line: &str| {
                recv.lock().unwrap().push(line.trim().to_string());
            };

            // Banner
            write!(w, "220 mock.smtp.test\r\n").unwrap();

            // EHLO
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            record(&recv, &line);
            write!(w, "250-mock.smtp.test Hello\r\n").unwrap();
            if with_auth {
                write!(w, "250 AUTH PLAIN\r\n").unwrap();
            } else {
                write!(w, "250 OK\r\n").unwrap();
            }

            // Optional AUTH PLAIN
            if with_auth {
                let mut auth = String::new();
                r.read_line(&mut auth).unwrap();
                record(&recv, &auth);
                write!(w, "235 Authentication successful\r\n").unwrap();
            }

            // MAIL FROM
            let mut mail = String::new();
            r.read_line(&mut mail).unwrap();
            record(&recv, &mail);
            write!(w, "250 OK\r\n").unwrap();

            // One or more RCPT TO
            loop {
                let mut rcpt = String::new();
                r.read_line(&mut rcpt).unwrap();
                record(&recv, &rcpt);
                let upper = rcpt.trim().to_uppercase();
                if upper.starts_with("RCPT TO") {
                    write!(w, "250 OK\r\n").unwrap();
                } else {
                    // DATA command
                    write!(w, "354 End data with CRLF.CRLF\r\n").unwrap();
                    break;
                }
            }

            // Consume message body until lone `.'
            loop {
                let mut body = String::new();
                r.read_line(&mut body).unwrap();
                if body.trim() == "." { break; }
            }
            write!(w, "250 OK: message accepted\r\n").unwrap();

            // QUIT
            let mut quit = String::new();
            r.read_line(&mut quit).unwrap();
            record(&recv, &quit);
            write!(w, "221 Bye\r\n").unwrap();
        });
        port
    }

    // ── EmailBuilder tests ────────────────────────────────────────────────────

    #[test]
    fn builder_requires_to_recipient() {
        let err = Email::builder().subject("Hi").text("body").build().unwrap_err();
        assert!(err.to_string().contains("recipient"), "{err}");
    }

    #[test]
    fn builder_requires_subject() {
        let err = Email::builder().to("a@b.com").text("body").build().unwrap_err();
        assert!(err.to_string().contains("subject"), "{err}");
    }

    #[test]
    fn builder_requires_body() {
        let err = Email::builder().to("a@b.com").subject("Hi").build().unwrap_err();
        assert!(err.to_string().contains("body"), "{err}");
    }

    #[test]
    fn builder_success_plain_text() {
        let email = Email::builder()
            .to("user@example.com")
            .subject("Hello")
            .text("World")
            .build()
            .unwrap();
        assert_eq!(email.to, vec!["user@example.com"]);
        assert_eq!(email.subject, "Hello");
        assert_eq!(email.text, Some("World".to_string()));
        assert!(email.html.is_none());
    }

    #[test]
    fn builder_multiple_recipients() {
        let email = Email::builder()
            .to("a@x.com")
            .to("b@x.com")
            .cc("c@x.com")
            .bcc("d@x.com")
            .subject("Multi")
            .html("<p>hi</p>")
            .build()
            .unwrap();
        assert_eq!(email.to.len(), 2);
        assert_eq!(email.cc, vec!["c@x.com"]);
        assert_eq!(email.bcc, vec!["d@x.com"]);
    }

    // ── dot_stuff tests ───────────────────────────────────────────────────────

    #[test]
    fn dot_stuff_escapes_leading_dot() {
        let out = dot_stuff(".hidden\nnormal\n..double");
        let lines: Vec<&str> = out.split("\r\n").collect();
        assert_eq!(lines[0], "..hidden",  "single dot → double dot");
        assert_eq!(lines[1], "normal",    "normal line unchanged");
        assert_eq!(lines[2], "...double", "double dot → triple dot");
    }

    #[test]
    fn dot_stuff_no_change_for_normal_text() {
        let input = "Hello World\r\nLine 2";
        let out = dot_stuff(input);
        assert!(!out.contains(".."));
        assert!(out.contains("Hello World"));
    }

    // ── build_message tests ───────────────────────────────────────────────────

    #[test]
    fn build_message_plain_text_headers() {
        let email = Email::builder()
            .to("dest@example.com")
            .subject("Test subject")
            .text("body text")
            .build()
            .unwrap();
        let msg = build_message("sender@example.com", &email);
        assert!(msg.contains("From: sender@example.com\r\n"), "From header");
        assert!(msg.contains("To: dest@example.com\r\n"), "To header");
        assert!(msg.contains("Subject: Test subject\r\n"), "Subject header");
        assert!(msg.contains("Content-Type: text/plain;"), "Content-Type");
        assert!(msg.contains("body text"), "body present");
    }

    #[test]
    fn build_message_html_only() {
        let email = Email::builder()
            .to("a@b.com")
            .subject("HTML")
            .html("<b>bold</b>")
            .build()
            .unwrap();
        let msg = build_message("from@b.com", &email);
        assert!(msg.contains("Content-Type: text/html;"));
        assert!(msg.contains("<b>bold</b>"));
    }

    #[test]
    fn build_message_multipart_alternative() {
        let email = Email::builder()
            .to("a@b.com")
            .subject("Both")
            .text("plain part")
            .html("<p>html part</p>")
            .build()
            .unwrap();
        let msg = build_message("from@b.com", &email);
        assert!(msg.contains("multipart/alternative"), "multipart content type");
        assert!(msg.contains("plain part"), "text part present");
        assert!(msg.contains("<p>html part</p>"), "html part present");
        assert!(msg.contains("text/plain;"), "text mime type");
        assert!(msg.contains("text/html;"), "html mime type");
    }

    // ── MailerError display ───────────────────────────────────────────────────

    #[test]
    fn mailer_error_display() {
        let e = MailerError::MissingConfig("RWS_SMTP_HOST".into());
        assert!(e.to_string().contains("RWS_SMTP_HOST"), "{e}");

        let e = MailerError::Smtp("550 User unknown".into());
        assert!(e.to_string().contains("550 User unknown"), "{e}");

        let e = MailerError::Build("subject is required".into());
        assert!(e.to_string().contains("subject"), "{e}");
    }

    // ── SMTP send tests (mock server) ─────────────────────────────────────────

    #[test]
    fn send_plain_smtp_no_auth() {
        let recv = Arc::new(Mutex::new(Vec::new()));
        let port = start_mock_smtp(recv.clone(), false);

        let mailer = Mailer {
            host: "127.0.0.1".into(),
            port,
            user: None,
            password: None,
            from: "from@example.com".into(),
            tls: SmtpTls::None,
            timeout_ms: 5_000,
        };
        let email = Email::builder()
            .to("to@example.com")
            .subject("Test")
            .text("Hello!")
            .build()
            .unwrap();

        mailer.send(&email).unwrap();

        let cmds = recv.lock().unwrap();
        assert!(cmds.iter().any(|l| l.to_uppercase().starts_with("EHLO")),    "EHLO sent");
        assert!(cmds.iter().any(|l| l.to_uppercase().starts_with("MAIL FROM")), "MAIL FROM sent");
        assert!(cmds.iter().any(|l| l.to_uppercase().starts_with("RCPT TO")),   "RCPT TO sent");
        assert!(cmds.iter().any(|l| l.to_uppercase() == "DATA"),                "DATA sent");
        assert!(cmds.iter().any(|l| l.to_uppercase() == "QUIT"),                "QUIT sent");
    }

    #[test]
    fn send_plain_smtp_with_auth() {
        let recv = Arc::new(Mutex::new(Vec::new()));
        let port = start_mock_smtp(recv.clone(), true);

        let mailer = Mailer {
            host: "127.0.0.1".into(),
            port,
            user: Some("user@test.com".into()),
            password: Some("s3cr3t".into()),
            from: "user@test.com".into(),
            tls: SmtpTls::None,
            timeout_ms: 5_000,
        };
        let email = Email::builder()
            .to("dest@example.com")
            .subject("Auth test")
            .text("Authenticated!")
            .build()
            .unwrap();

        mailer.send(&email).unwrap();

        let cmds = recv.lock().unwrap();
        assert!(
            cmds.iter().any(|l| l.to_uppercase().starts_with("AUTH PLAIN")),
            "AUTH PLAIN sent; got: {cmds:?}"
        );
    }

    #[test]
    fn send_multiple_recipients() {
        let recv = Arc::new(Mutex::new(Vec::new()));
        let port = start_mock_smtp(recv.clone(), false);

        let mailer = Mailer {
            host: "127.0.0.1".into(),
            port,
            user: None,
            password: None,
            from: "sender@example.com".into(),
            tls: SmtpTls::None,
            timeout_ms: 5_000,
        };
        let email = Email::builder()
            .to("a@example.com")
            .to("b@example.com")
            .subject("Multi")
            .text("hi")
            .build()
            .unwrap();

        mailer.send(&email).unwrap();

        let cmds = recv.lock().unwrap();
        let rcpt_count = cmds.iter()
            .filter(|l| l.to_uppercase().starts_with("RCPT TO"))
            .count();
        assert_eq!(rcpt_count, 2, "two RCPT TO commands; got: {cmds:?}");
    }
}
