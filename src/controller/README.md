[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Controller 

# Controller 

Controller module is designed to perform an action for a specific request.

Controller does two things: checks whether it is applicable for request and, if it is applicable, performs the action.

### Usage
[IndexController](https://github.com/bohdaq/rust-web-server/blob/main/src/controller/example/mod.rs) responsibility is to prepare http response containing `index.html` page. First you define struct (line [10](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L10)).
 
As we are going to use the name of index file, we defined the `INDEX_FILEPATH` constant inside `IndexController` implementation (line [13](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L13)).

#### Matching the request

To make our struct a `Controller`, we need to implement Controller trait (line [16](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L16)). Matching request starts with a slash and has GET method (line [18](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L18)). 

Matching can be done using any request field and connection info supplied to a function as parameters. Connection info contains ip and port for a client and server. Request contains http version, method, uri, headers and body.

As you can see `is_matching` method does return a boolean. It is done without explicitly using `return` keyword (line [18](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L18)), as at the end of line there is no a semicolon, which means the line returns result of evaluation (boolean) to the outer context (`is_matching` function). Such line is called `expression`. If you add a semicolon at the end of the line, it will not return a result of evaluation to the outer context, such line is called `statement`. Statement always returns empty result tuple  `()`, called [unit](https://doc.rust-lang.org/std/primitive.unit.html).

#### Preparing the response




#### Links
- [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
- [Response](https://github.com/bohdaq/rust-web-server/tree/main/src/response)
- [Server](https://github.com/bohdaq/rust-web-server/tree/main/src/server)
- [Application](https://github.com/bohdaq/rust-web-server/tree/main/src/application)
