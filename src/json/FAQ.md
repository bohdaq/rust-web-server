[Read Me](README.md) > JSON FAQ

# JSON Frequently Asked Questions

## Problem #1 
While working with json placed in separate file I'm getting error:
> thread 'json::object::test::deserialize_json_to_struct_another_example::deserialize_json_to_struct_another_example' panicked at 'assertion failed: `(left == right)`

> left: `"{\n  \"prop_a\": \"123abc\",\n  \"prop_b\": true,\n  \"prop_c\": false,\n  \"prop_d\": 4356257,\n  \"prop_e\": 4356.257\n}"`,

> right: `"{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257\r\n}"`', src/json/object/test/deserialize_json_to_struct_another_example/mod.rs:26:5

### Solution
JSON package doesn't take into account what type of new lines is used (`\n` or `\r\n`), or even if new lines are used at all.
However, it is common to use `\r\n` as a new line across `rust-web-server`.
Make sure while working with text files to set new lines to be `\r\n` in your editor of choice.


## Problem #2
While using `to_json_string` method I get not properly formatted (indented) json output. Why is so?

### Solution
JSON itself doesn't care about indentation. It is designed to transfer data, not represent it to the end user.
With this in mind `to_json_string` does not perform indentation formatting for nested objects and arrays.

However, it adds `\r\n` new lines and whitespace at a new property, object and array.
You can always take the output and reformat json manually using your IDE or online services.

## Problem #3
Why do I need to implement `New` trait?

### Solution
`New` trait is required if you're planning to use your struct in array. In most of the cases you are. Internally JSON module needs a way to instantiate a struct while working with json array, this functionality is achieved through `New` trait.

## Problem #4
I have a json list `[1, true, "text"]`, how can I parse it?

### Solution
As JSON module works with structs, there is _no way_ to logically map json array of different types (number, boolean and string) to list of structs of a particular type.

If you have such need, most likely, you're trying to describe struct itself containing these types. So, after remodeling, array of different types becomes array of structs containing these types as fields `[{1, true, "text"}]`.

Or as a workaround you may try to call `RawUnprocessedJSONArray::split_into_vector_of_strings(json)` to retrieve a list with strings that can be individually parsed to specific type.

## Problem #5
I want to convert tuple to json or vice versa. How can I do it?

### Solution
Closest variant of a tuple in json is array, so you can apply same workaround as discussed in `Problem 4`. 

But generally it's more of an antipattern and is not recommended.

## Problem #6
I looked at source code, and it looks like some undergraduate student work, nested loops, a bunch of ifs, are you kidding?

### Solution
Such design is comparably easy to maintain and add iterative enhancements. So it's concise decision, liking you this or not. Also, there's a high probability such implementation has performance superiority over more traditional approach (not tested, just a hypothesis).

## Problem #7
Why are you using `i128` for integers and `f64` for floating point numbers? Is it possible to use `u8` `i8` e.g.?

### Solution
JSON doesn't know about different number types, the same as JavaScript does. So to keep compatability between various possible variants of software decision was made to use the biggest numbers available.

If you have strong requirement to use smaller numbers, you can change your implementation for FromJSON trait and add casting to specific type.

## Problem #8
Why do I need to use JSON module if I can use [serde](https://serde.rs/)?

### Solution

There are 2 reasons why JSON module is written:

- Rust Web Server doesn't use any 3rd party dependencies except standard Rust library, again, concise decision to have control over codebase
- Tools like `serde` hiding all the implementation details by forcing user to use [procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html) like: `#[serde(default)]` and it's totally fine, but on other hand, again, you don't have granular control over the process of conversion to and from json, and if you're developing something more complex than TODO app, later, it will become a bottleneck. So JSON module on other-hand provides a trait-like implementation with prebuilt JSON conversion functions that are invoked from `your` codebase. So during whole development phase you're in charge of the process. And I think it's a great advantage and totally costs a bit of boilerplate which is required to set JSON serialization.

## Problem #9
I have json with property `"prop_e": 4356.257`, when I parse this json to struct, the field has value of `4356.2569999999996`, why is so and how to eliminate this?

### Solution

It is a [general problem](https://www.youtube.com/watch?v=WJgLKO-qac0) among programming languages. The best option is to avoid floating points when possible. 

As an example, if you want to use floating point for money representation, let's say `$2 50 cents`, don't use floats like `2.5`, instead use `250` integer type and you'll be fine.

Another example, if you want to represent latitude or longitude, instead of using float point number, simply use 2 separate integers. So `49.842957` will become two numbers `49` and `842957`. 

Another way to work around this issue, is to round the number `let rounded : String = format!("{:.N}", 4356.2569999999996);`, where `N` is the number of digits after floating point.

## Problem #10
What's the difference between json array, list and vector?

### Solution

No difference. Across this module you may find `array`, `list`, and `vector`, it is basically the same.

## Problem #11
I want to use `enum` in my struct, how can I do it?

### Solution

There's no such thing like `enum` in json. So to work around this issue in your struct, you can define 2 properties, one of integer or string type, and second of enum type. During parsing or converting simply read or set another property of enum type.