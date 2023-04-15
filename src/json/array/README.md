[Read Me](README.md) > [JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json) > Array

# JSON Array

JSON array module designed to parse raw json array (like `[ { "somekey": "text", "anotherkey": 1234 } ]`) to list of struct instances.

First you need to design your [struct](https://github.com/bohdaq/rust-web-server/blob/main/src/json/object/tests/example_multi_nested_object/some_object.rs#L8) by specifying fields. Then you need to implement [New](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L18), [FromJSON](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L32) and [ToJSON](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L118) traits for the struct.

Additionally, you specify [parse_json](https://github.com/bohdaq/rust-web-server/blob/main/src/json/object/tests/example_multi_nested_object/some_object.rs#L185), [to_json_list](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L213) and  [from_json_list](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/example_object.rs#L219) methods. 

To parse json, invoke [from_json_list](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/mod.rs#L71) method. 

To convert struct to JSON, invoke [to_json_list](https://github.com/bohdaq/rust-web-server/blob/main/src/json/array/object/example_multi_nested_object/mod.rs#L55) method.

