use crate::null::{Null, NULL};

#[test]
fn null_check() {

    let clone = NULL.clone();
    assert_eq!(*NULL, clone);

    let to_string = NULL.to_string();
    assert_eq!("null".to_string(), to_string);

    let debug = format!("{:?}", NULL);
    assert_eq!("null".to_string(), debug);

    let parsed : Null = "null".parse::<Null>().unwrap();
    assert_eq!(parsed, *NULL);

    let parse_error = "notnull".parse::<Null>();
    assert!(parse_error.is_err());

    let error_message = parse_error.err().unwrap();
    assert_eq!("error parsing null: notnull", error_message.message);

}