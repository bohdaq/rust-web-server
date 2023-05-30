[Read Me](https://github.com/bohdaq/rust-web-server/blob/main/README.md) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md)  > Range

# Range 

Response may contain several different parts of the same resource.

 Such functionality achieved through [range requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests).

Response body is a vector of a ContentRange instances.

### Structs

`Range` defines starting and ending byte.

`ContentRange` contains previously defined `Range`, length of bytes (end byte - start byte) as well as content-type for the given sequence of bytes (for example text, image, video) and measuring unit described as bytes.

### Functions

`get_content_range_of_a_file` returns `ContentRange` for a whole file.

`get_content_range` returns `ContentRange` for a given sequence of bytes and mime type.

Previous topic | Current Topic | Next Topic
--- |---------------| ---
TODO | TODO          | TODO       


