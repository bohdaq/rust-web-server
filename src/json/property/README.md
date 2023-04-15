[Read Me](README.md) > [JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json) > Property

# JSON Property

JSON property designed to parse raw json property (like `"somekey": "text"`) to JSONProperty and JSONValue instances.

JSONProperty contains two fields `property_name` and `property_type`.

JSONValue is a container type for the parsed json. It can be either `String`, `bool`, `object`, `array`, `i128 (integer)`, `f64 (floating point number)` or `null`.



