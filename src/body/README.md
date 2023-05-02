[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Body 

# Body 
Body is part of [request](https://github.com/bohdaq/rust-web-server/blob/fd45e7842ff66c85454e772c1f782da28d8166cb/src/request/mod.rs#L21) and [response](https://github.com/bohdaq/rust-web-server/blob/fd45e7842ff66c85454e772c1f782da28d8166cb/src/response/mod.rs#L28). It goes after the last header (if any present) and an empty line. 

### High level overview
Body is an arbitrary sequence of bytes (array of bytes `Vec<u8>` in request).

In response, it is represented via an array of ContentRange (`Vec<ContentRange>`) because response can contain several bodies if `multipart/byteranges` content type is set. Usually response does not contain multiple bodies, so the size of vector is one.






### Usage



#### Links
- [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
- [Response](https://github.com/bohdaq/rust-web-server/tree/main/src/response)
- [Server](https://github.com/bohdaq/rust-web-server/tree/main/src/server)
- [Controller](https://github.com/bohdaq/rust-web-server/tree/main/src/controller)
