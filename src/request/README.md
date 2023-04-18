[Read Me](README.md) > Request 

# Request 

Request module is designed to convert raw array of bytes to `Request` struct and vice versa.

### High level HTTP request overview
Example HTTP Request:
>HTTP/1.1 GET /static/style.css  
>Host: 127.0.0.1:7888
>
>Request Body Here

Where `HTTP/1.1 GET /static/style.css` is request line. It consists of `method` _GET_, `path ` _/static/style.css_ and `HTTP version` _HTTP/1.1 GET_.

After request line usually comes list of http `headers`, in this example it is _Host: 127.0.0.1:7888_. 

Each `header` (Host: 127.0.0.1:7888) starts with new line and contains header name (Host) followed by `:` and header value (127.0.0.1:7888).

Depending on HTTP version `headers` can be empty (prior to HTTP/1.1) or at least _Host_ header needs to be specified.
