use super::super::gcs_credentials::{self, Source};
use super::{GcsConfig, GcsStorage};
use crate::storage::Storage;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// A server answering one request per connection with `replies` in turn
/// (status, body), recording each request.
fn mock(replies: Vec<(u16, &'static str)>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for (status, reply_body) in replies {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    return;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    break p + 4;
                }
            };
            let head = String::from_utf8_lossy(&data[..head_end]).to_string();
            let mut lines = head.lines();
            let mut first = lines.next().unwrap_or("").split_whitespace();
            let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
            let headers: Vec<(String, String)> =
                lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
            let length = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0usize);
            while data.len() < head_end + length {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
            }
            let body = data[head_end..(head_end + length).min(data.len())].to_vec();
            log.lock().unwrap().push(Seen { method, target, headers, body });
            let reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply_body}", reply_body.len());
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    (base, seen)
}

fn storage(endpoint: &str, source: Source) -> GcsStorage {
    GcsStorage::with_source(GcsConfig { bucket: "my-bucket".to_string(), endpoint: endpoint.to_string() }, source)
}

#[test]
fn put_get_delete_use_the_json_api_with_the_token() {
    let (base, seen) = mock(vec![(200, r#"{"name":"a b/c.png"}"#), (200, "png bytes"), (204, ""), (404, r#"{"error":{}}"#)]);
    let store = storage(&base, Source::Token("ya29.token".to_string()));

    assert_eq!("a b/c.png", store.put("a b/c.png", b"png bytes", "image/png").unwrap());
    assert_eq!(b"png bytes".to_vec(), store.get("a b/c.png").unwrap());
    store.delete("a b/c.png").unwrap();
    store.delete("a b/c.png").unwrap(); // already gone: still fine

    let seen = seen.lock().unwrap();
    assert_eq!(("POST", "/upload/storage/v1/b/my-bucket/o?uploadType=media&name=a%20b%2Fc.png"), (seen[0].method.as_str(), seen[0].target.as_str()));
    assert_eq!(Some("image/png"), seen[0].header("content-type"));
    assert_eq!(b"png bytes".to_vec(), seen[0].body);
    assert_eq!(("GET", "/storage/v1/b/my-bucket/o/a%20b%2Fc.png?alt=media"), (seen[1].method.as_str(), seen[1].target.as_str()));
    assert_eq!(("DELETE", "/storage/v1/b/my-bucket/o/a%20b%2Fc.png"), (seen[2].method.as_str(), seen[2].target.as_str()));
    for request in seen.iter() {
        assert_eq!(Some("Bearer ya29.token"), request.header("authorization"));
    }
}

#[test]
fn errors_name_the_object_and_status() {
    let (base, _) = mock(vec![(404, r#"{"error":{"code":404}}"#), (403, "denied")]);
    let store = storage(&base, Source::Anonymous);
    let e = store.get("missing.txt").unwrap_err().to_string();
    assert!(e.contains("missing.txt") && e.contains("404"), "{e}");
    let e = store.delete("locked.txt").unwrap_err().to_string();
    assert!(e.contains("403"), "{e}");
}

#[test]
fn anonymous_sends_no_authorization_and_urls_are_public_ones() {
    let (base, seen) = mock(vec![(200, "{}")]);
    let store = storage(&base, Source::Anonymous);
    store.put("x.txt", b"x", "").unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(None, seen[0].header("authorization"));
    assert_eq!(Some("application/octet-stream"), seen[0].header("content-type"));
    let public = storage("https://storage.googleapis.com/", Source::Anonymous);
    assert_eq!("https://storage.googleapis.com/my-bucket/avatars/a%20b.png", public.url("avatars/a b.png"));
}

fn pkcs8_pem(der: &[u8]) -> String {
    let body = crate::core::base64::Base64::encode(der).unwrap();
    let lines: Vec<&str> = body.as_bytes().chunks(64).map(|c| std::str::from_utf8(c).unwrap()).collect();
    format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", lines.join("\n"))
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('"', "\\\"")
}

fn form_value<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    body.split('&').find_map(|pair| pair.strip_prefix(&format!("{name}=")))
}

fn base64url_decode(s: &str) -> Vec<u8> {
    let standard: String = s.chars().map(|c| match c { '-' => '+', '_' => '/', c => c }).collect();
    let padded = format!("{standard}{}", "=".repeat((4 - standard.len() % 4) % 4));
    crate::core::base64::Base64::decode(padded).unwrap()
}

#[test]
fn a_service_account_key_is_traded_for_a_cached_token() {
    use aws_lc_rs::encoding::AsDer;
    use aws_lc_rs::signature::KeyPair;
    let key = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
    let pkcs8: aws_lc_rs::encoding::Pkcs8V1Der<'static> = key.as_der().unwrap();

    let (base, seen) = mock(vec![(200, r#"{"access_token":"ya29.minted","expires_in":3599,"token_type":"Bearer"}"#), (200, "{}"), (200, "{}")]);
    let file = crate::test_env::TempPath::new("gcs_service_account.json");
    std::fs::write(
        &*file,
        format!(
            r#"{{"type":"service_account","client_email":"uploader@proj.iam.gserviceaccount.com","private_key":"{}","token_uri":"{base}/token"}}"#,
            json_escape(&pkcs8_pem(pkcs8.as_ref()))
        ),
    )
    .unwrap();
    let store = storage(&base, Source::File(file.to_path_buf()));
    store.put("a.txt", b"a", "text/plain").unwrap();
    store.put("b.txt", b"b", "text/plain").unwrap(); // the cached token, no second exchange

    let seen = seen.lock().unwrap();
    assert_eq!(3, seen.len());
    assert_eq!(("POST", "/token"), (seen[0].method.as_str(), seen[0].target.as_str()));
    let form = String::from_utf8(seen[0].body.clone()).unwrap();
    assert_eq!(Some("urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer"), form_value(&form, "grant_type"));
    let jwt = form_value(&form, "assertion").unwrap();
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(3, parts.len());
    let claims = String::from_utf8(base64url_decode(parts[1])).unwrap();
    assert!(claims.contains(r#""iss":"uploader@proj.iam.gserviceaccount.com""#), "{claims}");
    assert!(claims.contains(&format!(r#""aud":"{base}/token""#)) && claims.contains(gcs_credentials::SCOPE), "{claims}");
    let public = aws_lc_rs::signature::UnparsedPublicKey::new(&aws_lc_rs::signature::RSA_PKCS1_2048_8192_SHA256, key.public_key().as_ref().to_vec());
    public.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &base64url_decode(parts[2])).expect("signed with the service account's key");
    assert_eq!(Some("Bearer ya29.minted"), seen[1].header("authorization"));
    assert_eq!(Some("Bearer ya29.minted"), seen[2].header("authorization"));
}

#[test]
fn an_authorized_user_file_uses_its_refresh_token() {
    let (base, seen) = mock(vec![(200, r#"{"access_token":"ya29.user","expires_in":3600}"#), (200, "{}")]);
    let file = crate::test_env::TempPath::new("gcs_authorized_user.json");
    std::fs::write(
        &*file,
        format!(r#"{{"type":"authorized_user","client_id":"id.apps","client_secret":"s3cret","refresh_token":"1//refresh","token_uri":"{base}/token"}}"#),
    )
    .unwrap();
    let store = storage(&base, Source::File(file.to_path_buf()));
    store.get("a.txt").unwrap();
    let seen = seen.lock().unwrap();
    let form = String::from_utf8(seen[0].body.clone()).unwrap();
    assert_eq!(Some("refresh_token"), form_value(&form, "grant_type"));
    assert_eq!(Some("1%2F%2Frefresh"), form_value(&form, "refresh_token"));
    assert_eq!(Some("Bearer ya29.user"), seen[1].header("authorization"));
}

#[test]
fn the_metadata_server_gives_the_token_on_google_cloud() {
    let (base, seen) = mock(vec![(200, r#"{"access_token":"ya29.vm","expires_in":1800,"token_type":"Bearer"}"#), (200, "{}")]);
    let host = base.trim_start_matches("http://").to_string();
    let store = storage(&base, Source::Metadata(host));
    store.delete("a.txt").unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!("/computeMetadata/v1/instance/service-accounts/default/token", seen[0].target);
    assert_eq!(Some("Google"), seen[0].header("metadata-flavor"));
    assert_eq!(Some("Bearer ya29.vm"), seen[1].header("authorization"));
}

#[test]
fn credential_problems_are_explained() {
    let unreachable = storage("http://127.0.0.1:9", Source::Metadata("127.0.0.1:9".to_string()));
    let e = unreachable.get("a").unwrap_err().to_string();
    assert!(e.contains("no credentials") && e.contains("GOOGLE_APPLICATION_CREDENTIALS"), "{e}");
    let file = crate::test_env::TempPath::new("gcs_external_account.json");
    std::fs::write(&*file, r#"{"type":"external_account"}"#).unwrap();
    let e = storage("http://127.0.0.1:9", Source::File(file.to_path_buf())).get("a").unwrap_err().to_string();
    assert!(e.contains("external_account") && e.contains("service_account"), "{e}");
    let e = storage("http://127.0.0.1:9", Source::Token(String::new())).get("a").unwrap_err().to_string();
    assert!(e.contains("RWS_GCS_ACCESS_TOKEN"), "{e}");
}
