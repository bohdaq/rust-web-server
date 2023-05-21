use crate::core::base64::Base64;

#[test]
fn encode() {
    let data = "M".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TQ==", encoded);

    //let decoded = Base64::decode(encoded);
    //assert_eq!("M".as_bytes().to_vec(), decoded);
}