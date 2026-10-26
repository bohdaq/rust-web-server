use super::*;

#[test]
fn primitives_and_containers() {
    assert_eq!(r#"{"type":"boolean"}"#, bool::json_schema());
    assert_eq!(r#"{"type":"integer","minimum":0}"#, u32::json_schema());
    assert_eq!(r#"{"type":"integer"}"#, i64::json_schema());
    assert_eq!(r#"{"type":"string"}"#, String::json_schema());
    assert_eq!(r#"{"type":"array","items":{"type":"number"}}"#, Vec::<f64>::json_schema());
    assert_eq!(r#"{"anyOf":[{"type":"string"},{"type":"null"}]}"#, Option::<String>::json_schema());
    assert!(Option::<String>::is_optional());
    assert!(!String::is_optional());
    assert!(Box::<Option<u8>>::is_optional());
    assert_eq!(
        r#"{"type":"object","additionalProperties":{"type":"boolean"}}"#,
        HashMap::<String, bool>::json_schema()
    );
    assert_eq!(r#"{"type":"array","items":{"type":"string"},"minItems":2,"maxItems":2}"#, <[String; 2]>::json_schema());
}

#[test]
fn object_builder_and_descriptions() {
    let mut schema = ObjectSchema::new("A \"thing\"");
    schema.property("id", &u64::json_schema(), "", true);
    schema.property("note", &Option::<String>::json_schema(), "Free text", false);
    let schema = schema.finish();
    assert_eq!(
        r#"{"description":"A \"thing\"","type":"object","properties":{"id":{"type":"integer","minimum":0},"note":{"description":"Free text","anyOf":[{"type":"string"},{"type":"null"}]}},"required":["id"]}"#,
        schema
    );
    assert!(is_object_schema(&schema));
    assert!(!is_object_schema(&String::json_schema()));
    assert_eq!(r#"{"type":"object","properties":{}}"#, ObjectSchema::new("").finish());
    assert_eq!(r#"{"description":"d"}"#, with_description("{}", "d"));
    assert_eq!(r#"{"type":"string","enum":["a","b"]}"#, string_enum("", &["a", "b"]));
}

struct Node;

impl JsonSchema for Node {
    fn json_schema() -> String {
        format!(r#"{{"type":"array","items":{}}}"#, nested::<Node>())
    }
}

#[test]
fn a_recursive_type_ends_in_any_value() {
    let schema = Node::json_schema();
    assert!(schema.contains("{}"));
    assert_eq!(MAX_DEPTH as usize, schema.matches(r#""type":"array""#).count() - 1);
    // The depth counter is restored afterwards.
    assert_eq!(r#"{"type":"array","items":{"type":"boolean"}}"#, Vec::<bool>::json_schema());
}

#[cfg(all(feature = "macros", feature = "serde"))]
mod derived {
    use super::super::{is_object_schema, JsonSchema};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    /// Checks `value` against `schema` — the subset `schema` emits — and
    /// returns the first mismatch.
    fn check(schema: &Value, value: &Value, at: &str) -> Result<(), String> {
        if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
            return if any_of.iter().any(|s| check(s, value, at).is_ok()) {
                Ok(())
            } else {
                Err(format!("{at}: {value} matches no anyOf branch"))
            };
        }
        if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
            if !allowed.contains(value) {
                return Err(format!("{at}: {value} not in {allowed:?}"));
            }
        }
        let Some(ty) = schema.get("type").and_then(Value::as_str) else { return Ok(()) };
        let fits = match ty {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            other => return Err(format!("{at}: unknown type {other}")),
        };
        if !fits {
            return Err(format!("{at}: {value} is not {ty}"));
        }
        if let Some(min) = schema.get("minimum").and_then(Value::as_f64) {
            if value.as_f64().is_some_and(|v| v < min) {
                return Err(format!("{at}: {value} < {min}"));
            }
        }
        if let (Some(items), Some(elements)) = (schema.get("items"), value.as_array()) {
            for (i, element) in elements.iter().enumerate() {
                check(items, element, &format!("{at}[{i}]"))?;
            }
        }
        if let Some(object) = value.as_object() {
            for name in schema.get("required").and_then(Value::as_array).into_iter().flatten() {
                let name = name.as_str().unwrap_or_default();
                if !object.contains_key(name) {
                    return Err(format!("{at}: missing required {name}"));
                }
            }
            let properties = schema.get("properties").and_then(Value::as_object);
            for (name, member) in object {
                match properties.and_then(|p| p.get(name)) {
                    Some(property) => check(property, member, &format!("{at}.{name}"))?,
                    None => match schema.get("additionalProperties") {
                        Some(extra) => check(extra, member, &format!("{at}.{name}"))?,
                        // The derive never emits additionalProperties for a
                        // struct: an undeclared member means a rename was
                        // missed.
                        None if properties.is_some() => return Err(format!("{at}: undeclared property {name}")),
                        None => {}
                    },
                }
            }
        }
        Ok(())
    }

    fn conforms<T: Serialize + JsonSchema>(value: &T) {
        let schema: Value = serde_json::from_str(&T::json_schema()).expect("schema is JSON");
        let json = serde_json::to_value(value).unwrap();
        check(&schema, &json, "$").unwrap_or_else(|e| panic!("{e}\nschema: {schema}\nvalue: {json}"));
    }

    #[derive(Serialize, Deserialize, crate::JsonSchema)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    enum Status {
        InStock,
        #[serde(rename = "gone")]
        Discontinued,
        BackOrdered,
    }

    /// One line item.
    #[derive(Serialize, Deserialize, crate::JsonSchema)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct LineItem {
        /// Stock keeping unit.
        sku_code: String,
        unit_price: f64,
        #[serde(rename = "qty")]
        quantity: u32,
    }

    /// An order.
    #[derive(Serialize, Deserialize, crate::JsonSchema)]
    #[serde(rename_all = "camelCase")]
    struct Order {
        order_id: u64,
        customer_note: Option<String>,
        line_items: Vec<LineItem>,
        status: Status,
        #[serde(skip)]
        #[allow(dead_code)]
        internal_flag: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(alias = "r#type")]
        r#type: String,
        extra: std::collections::BTreeMap<String, i32>,
    }

    #[derive(Serialize, crate::JsonSchema)]
    struct Wrapper<T> {
        inner: T,
    }

    fn order(note: Option<&str>, tags: &[&str], status: Status) -> Order {
        Order {
            order_id: 7,
            customer_note: note.map(str::to_string),
            line_items: vec![LineItem { sku_code: "A-1".into(), unit_price: 2.5, quantity: 3 }],
            status,
            internal_flag: true,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            r#type: "retail".into(),
            extra: [("k".to_string(), -1)].into_iter().collect(),
        }
    }

    #[test]
    fn serialized_values_conform_to_the_derived_schema() {
        conforms(&order(Some("leave at door"), &["gift"], Status::InStock));
        conforms(&order(None, &[], Status::Discontinued));
        conforms(&order(None, &[], Status::BackOrdered));
        conforms(&Wrapper { inner: vec![Status::InStock] });
        assert!(is_object_schema(&Order::json_schema()));
    }

    #[test]
    fn the_derived_schema_follows_serde_attributes() {
        let schema: Value = serde_json::from_str(&Order::json_schema()).unwrap();
        assert_eq!("An order.", schema["description"]);
        let properties = schema["properties"].as_object().unwrap();
        let names: Vec<&str> = properties.keys().map(String::as_str).collect();
        assert_eq!(vec!["customerNote", "extra", "lineItems", "orderId", "status", "tags", "type"], {
            let mut sorted = names.clone();
            sorted.sort();
            sorted
        });
        assert_eq!(
            vec!["orderId", "lineItems", "status", "type", "extra"],
            schema["required"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>()
        );
        assert_eq!(serde_json::json!(["IN_STOCK", "gone", "BACK_ORDERED"]), properties["status"]["enum"]);
        let item = &properties["lineItems"]["items"];
        assert_eq!("One line item.", item["description"]);
        assert_eq!("Stock keeping unit.", item["properties"]["skuCode"]["description"]);
        assert!(item["properties"]["qty"].is_object());
    }

    #[test]
    fn a_value_with_a_missed_rename_does_not_conform() {
        // The checker is strict enough to catch a schema that disagrees with
        // serde: snake_case JSON against the camelCase schema fails.
        let schema: Value = serde_json::from_str(&LineItem::json_schema()).unwrap();
        let snake = serde_json::json!({"sku_code": "A", "unitPrice": 1.0, "qty": 1});
        assert!(check(&schema, &snake, "$").is_err());
        let negative = serde_json::json!({"skuCode": "A", "unitPrice": 1.0, "qty": -1});
        assert!(check(&schema, &negative, "$").is_err());
    }
}
