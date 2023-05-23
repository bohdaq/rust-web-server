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

#[test]
fn encode_three_chars() {
    let data = "Man".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TWFu", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!("Man".as_bytes().to_vec(), decoded);
}

#[test]
fn basic_text_encode() {
    let data = "Many hands make light work.".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TWFueSBoYW5kcyBtYWtlIGxpZ2h0IHdvcmsu", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!(data, decoded);



}