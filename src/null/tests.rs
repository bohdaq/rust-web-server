use crate::null::Null;

#[test]
fn null() {
    let null = Null{};

    let clone = null.clone();

    let to_string = clone.to_string();
    assert_eq!("null".to_string(), to_string);

    let debug = format!("{:?}", null);
    assert_eq!("null".to_string(), debug);

    let parsed : Null = "null".parse::<Null>().unwrap();
    assert_eq!(parsed, Null{});

    let parse_error = "notnull".parse::<Null>();
    assert!(parse_error.is_err());

    let error_message = parse_error.err().unwrap();
    assert_eq!("error parsing null: notnull", error_message.message);

}