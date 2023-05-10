use crate::url::URL;

#[test]
fn encode_decode() {
    let component = "\r\n \"%%\" \r\n";
    let mut _result = URL::encode_uri_component(component);
    assert_eq!("%0D%0A%20%22%25%25%22%20%0D%0A", _result);
    _result = URL::decode_uri_component(_result.as_str());
    assert_eq!(component, _result);
}