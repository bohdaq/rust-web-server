use super::*;
use crate::header::Header;
use crate::storage::LocalStorage;
use crate::test_env::TempPath;

fn storage(dir: &TempPath) -> LocalStorage {
    LocalStorage::new(dir.to_str().unwrap()).with_base_url("/uploads")
}

#[test]
fn store_read_url_delete() {
    let dir = TempPath::new("attachment_store");
    let storage = storage(&dir);
    let a = Attachment::store(&storage, "/docs/", "../../etc/pass wd", "text/plain", b"hello").unwrap();
    assert!(a.key.starts_with("docs/") && a.key.ends_with("/pass_wd"), "{}", a.key);
    assert_eq!(("../../etc/pass wd", "text/plain", 5), (a.filename.as_str(), a.content_type.as_str(), a.size));
    assert_eq!(b"hello".to_vec(), a.read(&storage).unwrap());
    assert_eq!(format!("/uploads/{}", a.key), a.url(&storage));
    let b = Attachment::store(&storage, "", ".hidden", "text/plain", b"x").unwrap();
    assert!(b.key.ends_with("/hidden") && !b.key.starts_with('/'), "{}", b.key);
    assert_ne!(a.key, Attachment::store(&storage, "docs", "../../etc/pass wd", "text/plain", b"").unwrap().key);
    a.delete(&storage).unwrap();
    assert!(a.read(&storage).is_err());
}

#[test]
fn store_stream_counts_bytes() {
    let dir = TempPath::new("attachment_stream");
    let storage = storage(&dir);
    let mut reader = std::io::Cursor::new(vec![7u8; 70_000]);
    let a = Attachment::store_stream(&storage, "big", "blob.bin", "application/octet-stream", &mut reader).unwrap();
    assert_eq!(70_000, a.size);
    assert_eq!(70_000, a.read(&storage).unwrap().len());
}

#[test]
fn encode_decode_round_trip() {
    let a = Attachment { key: "a/b c&d=e".into(), filename: "Résumé (1).pdf".into(), content_type: "application/pdf".into(), size: 42 };
    let text = a.encode();
    assert!(!text.contains(' ') && text.starts_with("key=a%2Fb%20c%26d%3De&"), "{text}");
    assert_eq!(Some(a), Attachment::decode(&text));
    assert_eq!(None, Attachment::decode(""));
    assert_eq!(None, Attachment::decode("name=x"));
    assert_eq!(None, Attachment::decode("key=a&size=lots"));
    assert_eq!(None, Attachment::decode("key=%ZZ"));
    assert_eq!(None, Attachment::decode("key"));
}

fn multipart(body: &str) -> Request {
    Request {
        method: "POST".into(),
        request_uri: "/upload".into(),
        http_version: "HTTP/1.1".into(),
        headers: vec![Header::new("Content-Type", "multipart/form-data; boundary=XyZ")],
        body: body.replace('\n', "\r\n").into_bytes().into(),
    }
}

#[test]
fn from_multipart_stores_the_named_file() {
    let dir = TempPath::new("attachment_multipart");
    let storage = storage(&dir);
    let request = multipart(
        "--XyZ\nContent-Disposition: form-data; name=\"title\"\n\nHi\n--XyZ\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\nContent-Type: image/png\n\nPNGDATA\n--XyZ\nContent-Disposition: form-data; name=\"empty\"; filename=\"\"\nContent-Type: application/octet-stream\n\n\n--XyZ--\n",
    );
    let a = Attachment::from_multipart(&storage, "avatars", &request, "avatar").unwrap().unwrap();
    assert_eq!(("me.png", "image/png", b"PNGDATA".to_vec()), (a.filename.as_str(), a.content_type.as_str(), a.read(&storage).unwrap()));
    assert_eq!(None, Attachment::from_multipart(&storage, "x", &request, "missing").unwrap());
    assert_eq!(None, Attachment::from_multipart(&storage, "x", &request, "empty").unwrap());
    let not_multipart = Request { headers: vec![Header::new("Content-Type", "application/json")], ..multipart("") };
    assert!(Attachment::from_multipart(&storage, "x", &not_multipart, "avatar").is_err());
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
#[test]
fn attachment_is_a_column_value() {
    use crate::model::{FromColumn, ToColumn, Value};
    let a = Attachment { key: "k".into(), filename: "f".into(), content_type: "t".into(), size: 1 };
    assert_eq!(Some(a.clone()), Option::<Attachment>::from_column(a.to_column()).unwrap());
    assert_eq!(None, Option::<Attachment>::from_column(Value::Null).unwrap());
    assert!(Attachment::from_column(Value::Text("junk".into())).is_err());
    assert!(Attachment::from_column(Value::Int(1)).is_err());
}
