use super::*;

#[test]
fn uris_parse() {
    let u = Pkcs11Uri::parse("pkcs11:token=rws%20prod;object=web-key;type=private?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-source=env:HSM_PIN").unwrap();
    assert_eq!(u.token.as_deref(), Some("rws prod"));
    assert_eq!(u.object.as_deref(), Some("web-key"));
    assert_eq!(u.module_path, "/usr/lib/softhsm/libsofthsm2.so");
    assert_eq!(u.pin, Some(PinSource::Env("HSM_PIN".into())));
    let u = Pkcs11Uri::parse("PKCS11:id=%01%02?module-path=/m.so&pin-source=file:///run/secrets/pin").unwrap();
    assert_eq!((u.id, u.pin), (Some(vec![1, 2]), Some(PinSource::File("/run/secrets/pin".into()))));
    assert!(Pkcs11Uri::parse("pkcs11:object=k").unwrap_err().contains("module-path"));
    assert!(Pkcs11Uri::parse("pkcs11:token=t?module-path=/m.so").unwrap_err().contains("object="));
    assert!(Pkcs11Uri::parse("pkcs11:color=red;object=k?module-path=/m.so").unwrap_err().contains("unknown attribute"));
    assert!(!Pkcs11Uri::is_uri("/etc/rws/key.pem"));
}

#[test]
fn raw_ecdsa_signatures_become_der() {
    // r has its top bit set (needs a 0 pad), s has leading zeros (trimmed).
    let mut raw = vec![0x80u8; 32];
    raw.extend(std::iter::repeat_n(0u8, 30).chain([0x01, 0x02]));
    let der = ecdsa_der(&raw);
    assert_eq!(&der[..4], &[0x30, 2 + 33 + 2 + 2, 0x02, 33]);
    assert_eq!(der[4], 0);
    assert_eq!(&der[der.len() - 4..], &[0x02, 2, 0x01, 0x02]);
}

#[test]
fn a_missing_module_is_a_clear_error() {
    let e = signing_key("pkcs11:object=k?module-path=/nonexistent/libnothing.so").unwrap_err();
    assert!(e.contains("/nonexistent/libnothing.so"), "{e}");
}

/// Against SoftHSM2 when the test environment provides one (CI's `pkcs11`
/// job sets these after creating a token and a key).
#[test]
fn signs_with_a_softhsm_key_when_available() {
    let Ok(uri) = std::env::var("RWS_TEST_PKCS11_URI") else { return };
    let key = signing_key(&uri).expect("opens the key");
    let signer = key.choose_scheme(&[SignatureScheme::ECDSA_NISTP256_SHA256, SignatureScheme::RSA_PSS_SHA256]).expect("a scheme");
    let sig = signer.sign(b"hello").expect("signs");
    assert!(!sig.is_empty());
}

#[test]
fn percent_decoding_is_rfc_3986_bytes() {
    assert_eq!(pct("%01%ff%41"), vec![1, 0xff, b'A']);
    assert_eq!(pct("%u0041"), b"%u0041".to_vec(), "no %u escapes");
    assert_eq!(pct("a%2"), b"a%2".to_vec());
}

#[test]
fn pins_are_hidden_in_output() {
    let uri = "pkcs11:object=k?module-path=/m.so&pin-value=1234&x=1";
    assert_eq!(crate::entry_point::shown("RWS_CONFIG_TLS_KEY_FILE", uri), "pkcs11:object=k?module-path=/m.so&pin-value=(hidden)&x=1");
    assert_eq!(crate::entry_point::shown("RWS_CONFIG_TLS_KEY_FILE", "pkcs11:object=k?pin-value=9"), "pkcs11:object=k?pin-value=(hidden)");
    assert_eq!(crate::entry_point::shown("RWS_CONFIG_TLS_KEY_FILE", "/etc/key.pem"), "/etc/key.pem");
}

#[test]
fn signing_waits_never_panic_on_any_runtime() {
    // Outside tokio.
    assert_eq!(1, off_the_worker(|| 1));
    // A current-thread runtime (per-core): block_in_place would panic here.
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    assert_eq!(2, rt.block_on(async { off_the_worker(|| 2) }));
    // The multi-threaded runtime: a worker task, block_on's thread, the blocking pool.
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).build().unwrap();
    assert_eq!(3, rt.block_on(async { tokio::spawn(async { off_the_worker(|| 3) }).await.unwrap() }));
    assert_eq!(4, rt.block_on(async { off_the_worker(|| 4) }));
    assert_eq!(5, rt.block_on(async { tokio::task::spawn_blocking(|| off_the_worker(|| 5)).await.unwrap() }));
}

#[test]
fn a_slow_token_does_not_stall_the_shared_runtime() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    // One worker: without block_in_place the slow "signature" would hold it,
    // and the other task couldn't run until it finished.
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().unwrap();
    let other_ran = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&other_ran);
    let started = Instant::now();
    let ran_after = rt.block_on(async move {
        let slow = tokio::spawn(async { off_the_worker(|| std::thread::sleep(Duration::from_millis(800))) });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let other = tokio::spawn(async move { seen.store(true, Ordering::SeqCst) });
        other.await.unwrap();
        let at = started.elapsed();
        slow.await.unwrap();
        at
    });
    assert!(other_ran.load(Ordering::SeqCst));
    assert!(ran_after < Duration::from_millis(600), "the other task waited for the token: {ran_after:?}");
}
