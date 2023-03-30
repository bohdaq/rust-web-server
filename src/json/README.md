[Read Me](README.md) > JSON 

# JSON 

_Work in Progress_

JSON module is designed to convert JSON string into corresponding hierarchy of structs in Rust and vice versa.

JSON needs to be valid, the module itself is not formatter or validator. However, it will return an error if provided json string is not appropriate.

As it's up to developer to decide how to define structs, the underlying conversion is achieved by obligation for developer to implement `New`, `FromJSON` and `ToJSON` traits. 

Luckily, all internal cumbersome functionally is done by JSON module, developer simply needs to define properties and invoke corresponding functions within trait implementation.

JSON module supports nested objects and arrays.

Within module itself, in tests section you may find examples on how to use it. 

Examples:

- [Object](object/tests/deserialize_json_to_struct)
- [Nested Object](object/tests/deserialize_json_with_multiple_nested_objects_to_struct)
- Array TODO
- Nested Array TODO

Links:
- [FAQ](FAQ.md)



