use super::*;

#[test]
fn base64url_has_no_padding_and_the_url_alphabet() {
    assert_eq!("", base64url(b""));
    assert_eq!("Zg", base64url(b"f"));
    assert_eq!("Zm8", base64url(b"fo"));
    assert_eq!("Zm9v", base64url(b"foo"));
    assert_eq!("-_8", base64url(&[0xfb, 0xff]));
}

#[test]
fn pem_bodies_decode_and_garbage_is_refused() {
    assert_eq!(Some(b"hello world".to_vec()), pem_body("-----BEGIN PRIVATE KEY-----\naGVsbG8g\nd29ybGQ=\n-----END PRIVATE KEY-----\n"));
    assert_eq!(None, pem_body("not a pem"));
    assert_eq!(None, pem_body("-----BEGIN PRIVATE KEY-----\n!!!!\n-----END PRIVATE KEY-----"));
}

#[test]
fn detection_follows_application_default_credentials() {
    let _g = crate::test_env::lock();
    let names = ["RWS_GCS_CREDENTIAL_SOURCE", "RWS_GCS_ACCESS_TOKEN", "GOOGLE_APPLICATION_CREDENTIALS", "GCE_METADATA_HOST", "CLOUDSDK_CONFIG"];
    let saved: Vec<_> = names.iter().map(|n| (n, std::env::var_os(n))).collect();
    let empty = crate::test_env::TempPath::new("gcs_no_gcloud");
    std::fs::create_dir_all(&*empty).unwrap();
    for n in names {
        std::env::remove_var(n);
    }
    std::env::set_var("CLOUDSDK_CONFIG", &*empty);

    assert_eq!(Source::Metadata("metadata.google.internal".to_string()), detect());
    std::env::set_var("GCE_METADATA_HOST", "169.254.169.254");
    assert_eq!(Source::Metadata("169.254.169.254".to_string()), detect());
    std::fs::write(empty.join("application_default_credentials.json"), "{}").unwrap();
    assert_eq!(Source::File(empty.join("application_default_credentials.json")), detect());
    std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", "/keys/sa.json");
    assert_eq!(Source::File("/keys/sa.json".into()), detect());
    std::env::set_var("RWS_GCS_ACCESS_TOKEN", "ya29.x");
    assert_eq!(Source::Token("ya29.x".to_string()), detect());
    std::env::set_var("RWS_GCS_CREDENTIAL_SOURCE", "none");
    assert_eq!(Source::Anonymous, detect());
    std::env::set_var("RWS_GCS_CREDENTIAL_SOURCE", "metadata");
    assert_eq!(Source::Metadata("169.254.169.254".to_string()), detect());

    for (n, v) in saved {
        match v {
            Some(v) => std::env::set_var(n, v),
            None => std::env::remove_var(n),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_bad_service_account_key_is_explained() {
    let e = service_account_jwt("a@b", "-----BEGIN PRIVATE KEY-----\naGVsbG8=\n-----END PRIVATE KEY-----", "https://t", 0).unwrap_err().to_string();
    assert!(e.contains("can't be used"), "{e}");
    let e = service_account_jwt("", "x", "https://t", 0).unwrap_err().to_string();
    assert!(e.contains("client_email"), "{e}");
}
