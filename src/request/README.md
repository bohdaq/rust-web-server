[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > Request 

# Request 

Request module is designed to convert raw array of bytes to `Request` struct and vice versa.

### High level HTTP request overview
Example HTTP Request (1-4 are line numbers, not part of the request):
>1 HTTP/1.1 GET /static/style.css  
> 
>2 Host: 127.0.0.1:7888
> 
>3 
> 
>4 Request Body Here

Where `HTTP/1.1 GET /static/style.css` is request line. It consists of `method` _GET_, `path ` _/static/style.css_ and `HTTP version` _HTTP/1.1 GET_.

After request line usually comes list of http `headers`, in this example it is _Host: 127.0.0.1:7888_. 

Each `header` (Host: 127.0.0.1:7888) starts with new line and contains header name (Host) followed by `:` and header value (127.0.0.1:7888).

Depending on HTTP version `headers` can be empty (prior to HTTP/1.1) or at least _Host_ header needs to be specified.

After `header` goes empty new line. Up to this point all characters have to be [UTF-8](https://en.wikipedia.org/wiki/UTF-8) encoded without any extra [control characters](https://en.wikipedia.org/wiki/Control_character). 

Body (Request Body Here) is the arbitrary sequence (array) of bytes and goes after previously mentioned empty new line, often referred as payload. 

Even though initially HTTP protocol was designed to transfer text based information, request body can be any set of bytes from image, video, audio, etc.

### Usage
To parse byte array to [Request](https://github.com/bohdaq/rust-web-server/blob/main/src/request/mod.rs#L16), simply call [Request::parse](https://github.com/bohdaq/rust-web-server/blob/main/src/request/example/mod.rs#L19) method.

To convert request instance to byte array invoke [generate](https://github.com/bohdaq/rust-web-server/blob/main/src/request/example/mod.rs#L82) method.

#### Links
- [List of HTTP methods](https://github.com/bohdaq/rust-web-server/blob/main/src/request/mod.rs#L37)
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
