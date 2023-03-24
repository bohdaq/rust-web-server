[Read Me](README.md) > JSON FAQ

# JSON Frequently Asked Questions

## Problem #1 
While working with json placed in separate file I'm getting error:
> thread 'json::object::test::deserialize_json_to_struct_another_example::deserialize_json_to_struct_another_example' panicked at 'assertion failed: `(left == right)`

> left: `"{\n  \"prop_a\": \"123abc\",\n  \"prop_b\": true,\n  \"prop_c\": false,\n  \"prop_d\": 4356257,\n  \"prop_e\": 4356.257\n}"`,

> right: `"{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257\r\n}"`', src/json/object/test/deserialize_json_to_struct_another_example/mod.rs:26:5

### Solution
JSON package does not take into account what type of new lines is used (`\n` or `\r\n`), or even if new lines are used at all.
However, it is common to use `\r\n` as a new line across `rust-web-server`.
Make sure while working with text files to set new lines to be `\r\n` in your editor of choice.


## Problem #2
While using `to_json_string` method I get not properly formatted (indented) json output. Why is so?

### Solution
JSON itself does not care about indentation. It is designed to transfer data, not represent it to the end user.
With this in mind `to_json_string` does not perform indentation formatting for nested objects and arrays.

However, it adds `\r\n` new lines and whitespace at a new property, object and array.
You can always take the output and reformat json manually using your IDE or online services.