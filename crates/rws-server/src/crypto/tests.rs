use super::{generate_token, hash_password, verify_password};

#[test]
fn correct_password_verifies() {
    let hash = hash_password("correct horse battery staple").unwrap();
    assert!(verify_password("correct horse battery staple", &hash).unwrap());
}

#[test]
fn wrong_password_does_not_verify() {
    let hash = hash_password("secret").unwrap();
    assert!(!verify_password("wrongpassword", &hash).unwrap());
}

#[test]
fn same_password_produces_different_hashes() {
    let h1 = hash_password("abc").unwrap();
    let h2 = hash_password("abc").unwrap();
    assert_ne!(h1, h2, "random salts must produce distinct hashes");
    assert!(verify_password("abc", &h1).unwrap());
    assert!(verify_password("abc", &h2).unwrap());
}

#[test]
fn hash_is_argon2id_phc_format() {
    let hash = hash_password("test").unwrap();
    assert!(
        hash.starts_with("$argon2id$"),
        "expected Argon2id PHC string, got: {hash}"
    );
}

#[test]
fn invalid_hash_string_returns_error() {
    assert!(verify_password("password", "not-a-valid-hash").is_err());
    assert!(verify_password("password", "").is_err());
}


#[test]
fn empty_password_hashes_and_verifies() {
    let hash = hash_password("").unwrap();
    assert!(verify_password("", &hash).unwrap());
    assert!(!verify_password("notempty", &hash).unwrap());
}

#[test]
fn unicode_password() {
    let hash = hash_password("пароль🔑").unwrap();
    assert!(verify_password("пароль🔑", &hash).unwrap());
    assert!(!verify_password("password", &hash).unwrap());
}

#[test]
fn long_password() {
    let password = "a".repeat(1000);
    let hash = hash_password(&password).unwrap();
    assert!(verify_password(&password, &hash).unwrap());
    assert!(!verify_password("a", &hash).unwrap());
}

#[test]
fn generate_token_hex_length() {
    assert_eq!(generate_token(16).len(), 32);
    assert_eq!(generate_token(32).len(), 64);
    assert_eq!(generate_token(0).len(), 0);
}

#[test]
fn generate_token_is_lowercase_hex() {
    let token = generate_token(32);
    assert!(
        token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "token is not lowercase hex: {token}"
    );
}

#[test]
fn generate_token_is_random() {
    let t1 = generate_token(32);
    let t2 = generate_token(32);
    assert_ne!(t1, t2, "two 32-byte tokens collided — CSPRNG likely broken");
}
