use crate::url::URL;

#[test]
fn encode_decode() {
    let component = "\r\n \"%!#$&'()*++*)('&$#!%\" \r\n";
    let mut _result = URL::encode_uri_component(component);
    assert_eq!("%0D%0A%20%22%25%21%23%24%26%27%28%29%2A%2B%2B%2A%29%28%27%26%24%23%21%25%22%20%0D%0A", _result);
    _result = URL::decode_uri_component(_result.as_str());
    assert_eq!(component, _result);
}