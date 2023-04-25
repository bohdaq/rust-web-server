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

### Multipart response
Response may contain several bodies for different resources. Such functionality achieved through Range requests. 

Example `multipart range` HTTP Response (1-15 are line numbers, not part of the request):

>1 HTTP/1.1 200 OK
> 
>2 Host: localhost
> 
>3 Content-Type: multipart/byteranges; 
> boundary=String_separator
> 
>4
> 
>5 --String_separator
> 
>6 Content-Type:  text/plain
> 
>7 Content-Range:  bytes 0-9/9
> 
>8
> 
>9 some text
> 
>10 --String_separator
>
>11 Content-Type:  text/plain
> 
>12 Content-Range:  bytes 0-12/12
> 
>13
> 
>14 another text
> 
>15 --String_separator


Content-Type header (line 3) is indicating that response contains several parts(multipart/byteranges) and how they are separated via boundary (String_separator).

Empty line number 4 is common delimiter between headers and response body.

For multipart response, body starts with boundary (line 5), indicating first part. Fun fact, the boundary within response body has two additional hyphens.

Each part structure is similar to plain response without response line. 

`Content-Type` header shows type of data contained within the part. `Content-Range` header shows number of bytes (bytes 0-9/`9`) in the part and theirs position (bytes `0-9`/9) in the original file. 

Empty line (number 8) is delimiter between parts headers and body. First parts body (payload) starts immediately after, up to the next boundary.

Same process repeated for the second part.

Any other response except `multipart/byteranges` considered to contain only one part. 

### Usage
To convert response instance to byte array invoke [generate](https://github.com/bohdaq/rust-web-server/blob/161f6058d6c646d9ea89120b6aab861cdebf42d9/src/response/example/mod.rs#L90) method.

To parse byte array to [Response](https://github.com/bohdaq/rust-web-server/blob/71e4df81ed3b89807502df5897c84cfdbaebe94d/src/response/mod.rs#L23), simply call [Response::parse](https://github.com/bohdaq/rust-web-server/blob/161f6058d6c646d9ea89120b6aab861cdebf42d9/src/response/example/mod.rs#L17) method.

Response struct contains common fields representing http response, however, instead
 of the body field there is a list of ContentRange. 

In practice, it means when you work with any response, except for the `multipart/byteranges` Content-Type, the body of the request is contained as a `ContentRange` instance within `content_range_list` field.

When you are working with `multipart/byteranges` response, the `content_range_list` field will contain each individual part of the Range response. 

#### Links
- [List of status codes](https://github.com/bohdaq/rust-web-server/blob/2db963550eb254132566a316974223ba8981c6ff/src/response/mod.rs#L102)
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
- [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)




