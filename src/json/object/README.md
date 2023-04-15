[Read Me](README.md) > JSON > Object

# JSON Object

JSON object module designed to parse raw json object (like `{ "somekey": "text", "anotherkey": 1234 }`) to struct instance.

First you need to design your [struct](https://github.com/bohdaq/rust-web-server/blob/main/src/json/object/tests/example_multi_nested_object/some_object.rs#L8) by specifying fields. Then you need to implement [New](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L18), [FromJSON](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L32) and [ToJSON](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L118) traits for the struct.

Additionally, you specify [parse_json](https://github.com/bohdaq/rust-web-server/blob/main/src/json/object/tests/example_multi_nested_object/some_object.rs#L185), [to_json_list](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L213) and  [from_json_list](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L219) methods. 

To parse json, invoke [parse_json](https://github.com/bohdaq/rust-web-server/blob/main/src/json/object/tests/example_multi_nested_object/mod.rs#L23) method. 

To convert struct to JSON, invoke [to_json_string](https://github.com/bohdaq/rust-web-server/blob/main/src/json/object/tests/example_multi_nested_object/mod.rs#L57) method.

