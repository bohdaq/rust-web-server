use crate::header::Header;

#[test]
fn parse() {
    let name : String = "Custom-Header".to_string();
    let value : String = "some value".to_string();

    let header_string : String = format!("{}: {}", name, value);

    let parse_result = Header::parse(header_string.as_str());
    if parse_result.is_err() {
        let _message = parse_result.clone().err().unwrap();
        // handle error
    }

    let header = parse_result.unwrap();

    // asserts, replace with your logic
    assert_eq!(header.name, name);
    assert_eq!(header.value, value);
}

#[test]
fn build() {
    let header = Header {
        name: "Custom-Header".to_string(),
        value: "some value".to_string()
    };

    let header_string = header.generate();
    assert_eq!("Custom-Header: some value", header_string);
}