[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > Header 

# Header 

Header module is designed to convert string to `header` struct and vice versa.

### High level HTTP header overview
Example HTTP header:

> Host: 127.0.0.1:7888


Header (Host: 127.0.0.1:7888) starts with new line and contains header name (Host) followed by `:` and header value (127.0.0.1:7888).

Header needs to be UTF-8 encoded and does not contain any extra [control characters](https://en.wikipedia.org/wiki/Control_character).

There are predefined list of headers to cover most use cases. 

### Usage
To parse string to [Header](https://github.com/bohdaq/rust-web-server/blob/main/src/header/mod.rs#L18), simply call [Header::parse](https://github.com/bohdaq/rust-web-server/blob/main/src/header/example/mod.rs#L10) method.

To convert header instance to string invoke [generate](https://github.com/bohdaq/rust-web-server/blob/main/src/header/example/mod.rs#L30) method.

#### Links
- [List of HTTP headers](https://github.com/bohdaq/rust-web-server/blob/main/src/header/mod.rs#L30)
- [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)

