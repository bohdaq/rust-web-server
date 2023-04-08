[Read Me](README.md) > JSON 

# JSON 

JSON module is designed to convert JSON string into corresponding hierarchy of structs in Rust and vice versa.

JSON needs to be valid, the module itself is not formatter or validator. However, it will return an error if provided json string is not appropriate.

As it's up to developer to decide how to define structs, the underlying conversion is achieved by obligation for developer to implement `New`, `FromJSON` and `ToJSON` traits. 

Luckily, all internal cumbersome functionally is done by JSON module, developer simply needs to define properties and invoke corresponding functions within trait implementation.

JSON module supports nested objects and arrays.

Within module itself, in a tests section, you may find examples on how to use it. 

Examples:

- [Object](object/tests/example)
- [Nested Object](object/tests/example_multi_nested_object)
- [Array of Objects](array/tests/example)
- [Array of Objects with Nested Array](array/tests/example_multi_nested_object)
- [Array of i128](array/tests/example_list_i128)
- [Array of u128](array/tests/example_list_u128)
- [Array of null](array/tests/example_list_null)

Links:
- [FAQ](FAQ.md)



