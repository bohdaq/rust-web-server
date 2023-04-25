[Read Me](README.md) > JSON 

# JSON 

JSON module is designed to convert JSON string into corresponding hierarchy of structs in Rust and vice versa.

JSON needs to be valid, the module itself is not formatter or validator. However, it will return an error if provided json string is not appropriate.

As it's up to developer to decide how to define structs, the underlying conversion is achieved by obligation for developer to implement `New`, `FromJSON` and `ToJSON` traits. 

Luckily, all internal cumbersome functionally is done by JSON module, developer simply needs to define properties and invoke corresponding functions within trait implementation.

JSON module supports nested objects and arrays.

Within module itself, in a tests section, you may find examples on how to use it. 

### Array

Take a look at [json array](https://github.com/bohdaq/rust-web-server/tree/main/src/json/array) module.

### Object

Take a look at [json object](https://github.com/bohdaq/rust-web-server/tree/main/src/json/object) module.

### Property

Take a look at [json property](https://github.com/bohdaq/rust-web-server/tree/main/src/json/property) module.


#### Examples:

- [Object](object/tests/example)
- [Nested Object](object/tests/example_multi_nested_object)
- [Array of Objects](array/object/example_multi_nested_object)
- [Array of Objects with Nested Array](array/object/example_multi_nested_object)
- [Array of i128](array/integer/example_list_i128)
- [Array of i64](array/integer/example_list_i64)
- [Array of i32](array/integer/example_list_i32)
- [Array of i16](array/integer/example_list_i16)
- [Array of i8](array/integer/example_list_i8)
- [Array of u128](array/integer/example_list_u128)
- [Array of u64](array/integer/example_list_u64)
- [Array of u32](array/integer/example_list_u32)
- [Array of u16](array/integer/example_list_u16)
- [Array of u8](array/integer/example_list_u8)
- [Array of null](array/null/example_list_null)
- [Array of String](array/string/example_list_string)
- [Array of bool](array/boolean/example_list_bool)
- [Array of f64](array/float/example_list_f64)
- [Array of f32](array/float/example_list_f32)

Links:
- [FAQ](FAQ.md)



