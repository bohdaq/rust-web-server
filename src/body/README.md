[Read Me](https://github.com/bohdaq/rust-web-server/blob/main/README.md) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Body 

# Body 
Body is a part of [request](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/request/mod.rs#L21) and [response](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/response/mod.rs#L28). It goes after the last header (if any present) and an empty line.

### High level overview
Body is an arbitrary sequence of bytes (array of bytes `Vec<u8>` in request).

In response, it is represented via an array of ContentRange (`Vec<ContentRange>`) because response may contain several different parts of the same resource if `multipart/byteranges` content type is set. Usually response does not contain multiple bodies, so the size of vector is one.

`ContentRange` is a container struct for storing data and information about this data such as what part of originating file it is (either the file is sent fully or only a specific portion of the file is sent from byte M to byte N).

### Usage

Example on how to use raw body within [request](https://github.com/bohdaq/rust-web-server/blob/c0300d300c823a7f795ed65f28cab19000f7db98/src/body/example/mod.rs#L11) and [response](https://github.com/bohdaq/rust-web-server/blob/c0300d300c823a7f795ed65f28cab19000f7db98/src/body/example/mod.rs#L28). In case response body contains several parts, apply the same logic to each `ContentRange`.

Except raw bytes, body can be `application/x-www-form-urlencoded`, `multipart/form-data` or `application/json`.

#### Form Url Encoded 

Form Url Encoded request contains [url query](https://en.wikipedia.org/wiki/Query_string) string as body payload.

Example on how to use `application/x-www-form-urlencoded` body within [request](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/body/example/mod.rs#L201).

#### Multipart Form Data

Multipart form data request contains several parts of the same resource (`ContentRange`). Each part is an arbitrary sequence of bytes and consists of headers, where `Content-Disposition` header is mandatory and body containing the payload.

Example on how to use `multipart/form-data` body within [request](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/body/example/mod.rs#L69) and [response](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/body/example/mod.rs#L126).

How to [handle request](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/app/controller/form/multipart_enctype_post_method/mod.rs#L13) via controller.

How to [generate such request](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/request/tests.rs#L243).

#### JSON

How to [generate and parse](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/body/example/mod.rs#L234) JSON body in `application/json` request.

Same applies to [response](https://github.com/bohdaq/rust-web-server/blob/2a704b8d9b1278c6b2f28c543802e0ca9d943462/src/body/example/mod.rs#L282).

More on [handling JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json).

#### Multipart Response
Response may contain several different parts of the same resource. Such functionality achieved through Range requests.

Example `multipart/byteranges` [response body](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/body/example/mod.rs#L340). How raw `multipart/byteranges` [response](https://github.com/bohdaq/rust-web-server/blob/754e18a94548df7f4fb1fcebf3a6caddefb862cf/src/response/example/response.multipart.txt#L1) looks like.

#### Notes
- `HEAD` and `OPTIONS` request does not have body.

- `multipart/byteranges` applies only to response body.
- `multipart/form-data` and `application/x-www-form-urlencoded` applies only to request body



Previous topic | Current Topic | Next Topic
--- |---------------| ---
[Controller](https://github.com/bohdaq/rust-web-server/tree/main/src/controller) | Body          | [JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json)

