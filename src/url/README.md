[Read Me](https://github.com/bohdaq/rust-web-server/blob/main/README.md) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md)  > URL

# URL 

Module designed to provide convenient ways to work with [URL](https://en.wikipedia.org/wiki/URL).

### Percent encoding
Use `URL::percent_encode` and `URL::percent_decode` to [encode](https://github.com/bohdaq/rust-web-server/blob/18f0ec949fc744ee71a740f1098c8b2a5d0b50e8/src/url/example/mod.rs#L6) and [decode](https://github.com/bohdaq/rust-web-server/blob/18f0ec949fc744ee71a740f1098c8b2a5d0b50e8/src/url/example/mod.rs#L13) string.

### Query

Build(`URL::build_query`) and parse(`URL::parse_query`) URL query by using [build_query](https://github.com/bohdaq/rust-web-server/blob/18f0ec949fc744ee71a740f1098c8b2a5d0b50e8/src/url/example/mod.rs#L20) and [parse_query](https://github.com/bohdaq/rust-web-server/blob/18f0ec949fc744ee71a740f1098c8b2a5d0b50e8/src/url/example/mod.rs#L30) methods.

### URL

Build(`URL::build`) and parse(`URL::parse`) URL by using [build](https://github.com/bohdaq/rust-web-server/blob/18f0ec949fc744ee71a740f1098c8b2a5d0b50e8/src/url/example/mod.rs#L43) and [parse](https://github.com/bohdaq/rust-web-server/blob/18f0ec949fc744ee71a740f1098c8b2a5d0b50e8/src/url/example/mod.rs#L66) methods.

### FAQ

What problem does percent encoding solve?

To perform GET request with the query parameters specified, query parameters are appended to request_uri in request line (`GET /path?q=some_query HTTP/1.1`) If query (`q=some_query`) contains, for example, a whitespace (`q=some query`), the request will be malformed. To resolve such kind of issues percent encoding exists (`q=some&20query`).

Another use case is encoding `=` (encoded as `%23`) and `&` (encoded as `%26`). Ampersand is used as a separator between parameters `q=some query` and `key=1&=`  (`q=some&20query&key=1%26%23`). Equals used to indicate key-value pair in query.

Previous topic | Current Topic | Next Topic
--- |---------------| ---
[JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json) | URL           | [Null](https://github.com/bohdaq/rust-web-server/tree/main/src/null)



