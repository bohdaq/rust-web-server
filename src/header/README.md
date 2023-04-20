[Read Me](README.md) > Header 

# Header 

Header module is designed to convert string to `header` struct and vice versa.

### High level HTTP header overview
Example HTTP header:

> Host: 127.0.0.1:7888


Header (Host: 127.0.0.1:7888) starts with new line and contains header name (Host) followed by `:` and header value (127.0.0.1:7888).

Header needs to be UTF-8 encoded and does not contain any extra [control characters](https://en.wikipedia.org/wiki/Control_character).

There are predefined list of headers to cover most use cases. 

### Usage
To parse string to [Header](), simply call [Header::parse]() method.

To convert header instance to string invoke [generate]() method.

#### Links
- [List of HTTP headers]()

