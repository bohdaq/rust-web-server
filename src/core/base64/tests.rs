use crate::core::base64::Base64;

#[test]
fn encode_one_char() {
    let data = "M".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TQ==", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!("M".as_bytes().to_vec(), decoded);
}


#[test]
fn encode_two_chars() {
    let data = "Ma".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TWE=", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!("Ma".as_bytes().to_vec(), decoded);
}