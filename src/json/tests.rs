use crate::json::{FromAndToJSON, JSONProperty, JSONValue};

#[test]
fn parse() {
    struct SomeObject {
        propA: String,
        propB: bool
    }

    impl FromAndToJSON for SomeObject {
        fn list_properties() -> Vec<JSONProperty> {
            todo!()
        }

        fn get_property(property: JSONProperty) -> JSONValue {
            todo!()
        }

        fn to_json_string() -> String {
            todo!()
        }

        fn from_json_string(json_string: String) -> Self {
            todo!()
        }
    }
}