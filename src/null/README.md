[Read Me](README.md) > Null

# Null 

Representation of `null` in Rust. Initially needed by JSON module, but not limited to. 

Implements several Rust core traits:

- `FromStr`
- `Clone`
- `Debug`
- `PartialEq`
- `Display`

Additionaly implements Rust Web Server trait `New`.

In case of parse error returns `ParseNullError`. Error details specified within `message` field.



