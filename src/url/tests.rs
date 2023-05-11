use crate::url::URL;

#[test]
fn encode_decode() {
    let component = "\r\n \"%!#$&'()*+,/:;=?@[]][@?=;:/,+*)('&$#!%\" \r\n";
    let mut _result = URL::percent_encode(component);
    assert_eq!("%0D%0A%20%22%25%21%23%24%26%27%28%29%2A%2B%2C%2F%3A%3B%3D?%40%5B%5D%5D%5B%40?%3D%3B%3A%2F%2C%2B%2A%29%28%27%26%24%23%21%25%22%20%0D%0A", _result);
    _result = URL::percent_decode(_result.as_str());
    assert_eq!(component, _result);
}