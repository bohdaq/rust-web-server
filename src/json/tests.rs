use crate::json::{FromAndToJSON, JSONProperty, JSONValue};

#[test]
fn parse() {
    struct SomeObject {
        propA: String,
        propB: bool
    }

    impl FromAndToJSON for SomeObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "propA".to_string(), property_type: "String".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "propB".to_string(), property_type: "bool".to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property: JSONProperty) -> JSONValue {
            let mut value = JSONValue {
                i8: None,
                u8: None,
                i16: None,
                u16: None,
                i32: None,
                u32: None,
                i64: None,
                u64: None,
                i128: None,
                u128: None,
                usize: None,
                isize: None,
                string: None,
                boolean: None,
                null: None,
            };

            if property.property_name == "propA".to_string() {
                let string : String = self.propA.to_owned();
                value.string = Some(string);
            }

            if property.property_name == "propB".to_string() {
                let boolean : bool = self.propB;
                value.boolean = Some(boolean);
            }

            value
        }

        fn to_json_string() -> String {
            todo!()
        }

        fn from_json_string(json_string: String) -> Self {
            todo!()
        }
    }
}