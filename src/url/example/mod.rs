use crate::url::URL;

#[test]
fn uri_encode() {
    let text = "some text to encode &=";
    let encoded = URL::percent_encode(text);
    assert_eq!("some%20text%20to%20encode%20%26%3D", encoded);
}

#[test]
fn uri_decode() {
    let text = "some%20text%20to%20encode%20%26%3D";
    let decoded = URL::percent_decode(text);
    assert_eq!("some text to encode &=", decoded);
}
