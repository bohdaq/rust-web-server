use crate::json::{FromAndToJSON, JSONProperty, JSONValue};
use crate::symbol::SYMBOL;

#[test]
fn parse() {
    struct SomeObject {
        prop_a: String,
        prop_b: bool
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

        fn get_property(&self, property_name: String) -> JSONValue {
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

            if property_name == "propA".to_string() {
                let string : String = self.prop_a.to_owned();
                value.string = Some(string);
            }

            if property_name == "propB".to_string() {
                let boolean : bool = self.prop_b;
                value.boolean = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut json_list = vec![];
            json_list.push(SYMBOL.opening_curly_bracket.to_string());

            let properties = SomeObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());

                if &property.property_type == "String" {
                    let raw_value = value.string.unwrap();
                    let formatted_property = format!("  \"{}\": \"{}\"", &property.property_name, raw_value);
                    json_list.push(formatted_property.to_string());
                }

                if &property.property_type == "bool" {
                    let raw_value = value.boolean.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    json_list.push(formatted_property.to_string());
                }
            }
            json_list.push(SYMBOL.closing_curly_bracket.to_string());


            let json = json_list.join(SYMBOL.new_line_carriage_return);
            json
        }

        fn from_json_string(json_string: String) -> Self {
            todo!()
        }
    }

    let obj = SomeObject { prop_a: "123abc".to_string(), prop_b: true };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"propA\": \"123abc\"\r\n  \"propB\": true\r\n}";

    assert_eq!(expected_json_string, json_string)
}