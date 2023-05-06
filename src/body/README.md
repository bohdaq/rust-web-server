[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Body 

# Body 
Body is a part of [request](https://github.com/bohdaq/rust-web-server/blob/fd45e7842ff66c85454e772c1f782da28d8166cb/src/request/mod.rs#L21) and [response](https://github.com/bohdaq/rust-web-server/blob/fd45e7842ff66c85454e772c1f782da28d8166cb/src/response/mod.rs#L28). It goes after the last header (if any present) and an empty line. 

### High level overview
Body is an arbitrary sequence of bytes (array of bytes `Vec<u8>` in request).

In response, it is represented via an array of ContentRange (`Vec<ContentRange>`) because response can contain several bodies if `multipart/byteranges` content type is set. Usually response does not contain multiple bodies, so the size of vector is one.

`ContentRange` is a container struct for storing data and information about this data such as what part of originating file it is (either the file is sent fully or only a specific portion of the file is sent from byte M to byte N).

### Usage

Example on how to use raw body within [request](https://github.com/bohdaq/rust-web-server/blob/c0300d300c823a7f795ed65f28cab19000f7db98/src/body/example/mod.rs#L8) and [response](https://github.com/bohdaq/rust-web-server/blob/c0300d300c823a7f795ed65f28cab19000f7db98/src/body/example/mod.rs#L25). In case response body contains several parts, apply the same logic to each `ContentRange`.

Except raw bytes, body can be `application/x-www-form-urlencoded`, `multipart/form-data` or `application/json`.

#### Form Url Encoded 

Form Url Encoded request contains [url query](https://en.wikipedia.org/wiki/Query_string) string as body payload.

Example on how to use `application/x-www-form-urlencoded` body within [request](https://github.com/bohdaq/rust-web-server/blob/3c6d0aef9b02dfea97c97bd204df856f0a1ae73f/src/body/example/mod.rs#L194).

#### Multipart Form Data

Multipart form data request contains several parts of the same resource (`ContentRange`). Each part is an arbitrary sequence of bytes and consists of headers, where `Content-Disposition` header is mandatory and body containing the payload.

Example on how to use `multipart/form-data` body within [request](https://github.com/bohdaq/rust-web-server/blob/c0300d300c823a7f795ed65f28cab19000f7db98/src/body/example/mod.rs#L60) and [response](https://github.com/bohdaq/rust-web-server/blob/c0300d300c823a7f795ed65f28cab19000f7db98/src/body/example/mod.rs#L117).

How to [handle request](https://github.com/bohdaq/rust-web-server/blob/a0a304f7cfb3734fdbd56e304f771a9ac322e386/src/app/controller/form/multipart_enctype_post_method/mod.rs#L13) via controller.

How to [generate such request](https://github.com/bohdaq/rust-web-server/blob/a0a304f7cfb3734fdbd56e304f771a9ac322e386/src/request/tests.rs#L243).

#### JSON

How to [generate and parse](https://github.com/bohdaq/rust-web-server/blob/0d027c86e68511345e3bd9075f7b057f0e463b3d/src/body/example/mod.rs#L234) JSON body in `application/json` request.

Same applies to [response](https://github.com/bohdaq/rust-web-server/blob/938ddc5e946699f5d9e5a20b78241a296fbfd597/src/body/example/mod.rs#L282).

More on [handling JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json).


#### Links
- [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
- [Response](https://github.com/bohdaq/rust-web-server/tree/main/src/response)
- [Server](https://github.com/bohdaq/rust-web-server/tree/main/src/server)
- [Controller](https://github.com/bohdaq/rust-web-server/tree/main/src/controller)
