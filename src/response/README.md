[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > Response 

# Response 

Response module is designed to convert `Response` struct to raw array of bytes and vice versa.

### High level HTTP response overview
Example HTTP Response (1-6 are line numbers, not part of the request):

>1 HTTP/1.1 200 OK
>
>2 Host: localhost
> 
>3 Content-Range: bytes 0-9/9
> 
>4 Content-Length: 9
> 
>5
> 
>6 some text


Where `HTTP/1.1 200 OK` is response line. It consists of `HTTP version` _HTTP/1.1_, `status code ` _200_ and `reason phrase` _OK_.

After response line usually comes list of http `headers`, in this example they are _Host: 127.0.0.1:7888_, _Content-Range: bytes 0-9/9_ and _Content-Length: 9_.

Each `header` (Host: 127.0.0.1:7888) starts with new line and contains header name (Host) followed by `:` and header value (127.0.0.1:7888).

Depending on HTTP version `headers` can be empty (prior to HTTP/1.1) or at least _Host_ header needs to be specified.

After `header` goes empty new line. Up to this point all characters have to be [UTF-8](https://en.wikipedia.org/wiki/UTF-8) encoded without any extra [control characters](https://en.wikipedia.org/wiki/Control_character).

Body (Response Body Here) is the arbitrary sequence (array) of bytes and goes after previously mentioned empty new line, often referred as payload.

Even though initially HTTP protocol was designed to transfer text based information, request body can be any set of bytes from image, video, audio, etc.
