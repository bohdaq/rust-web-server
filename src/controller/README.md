[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Controller 

# Controller 

Controller module is designed to perform an action for a specific request.

Controller does two things: checks whether it is applicable for request and, if it is applicable, performs the action.

### Usage
[IndexController](https://github.com/bohdaq/rust-web-server/blob/main/src/controller/example/mod.rs) responsibility is to prepare http response containing `index.html` page. First you define struct (line [10](https://github.com/bohdaq/rust-web-server/blob/a3982dd08a85897d43280954d4e05567507b478f/src/controller/example/mod.rs#L10)).
 
As we are going to use the name of index file, we defined the `INDEX_FILEPATH` constant inside `IndexController` implementation (line [13](https://github.com/bohdaq/rust-web-server/blob/a3982dd08a85897d43280954d4e05567507b478f/src/controller/example/mod.rs#L13)).

To make our struct a `Controller`, we need to implement Controller trait (line [16](https://github.com/bohdaq/rust-web-server/blob/a3982dd08a85897d43280954d4e05567507b478f/src/controller/example/mod.rs#L16)). 




#### Links
- [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
- [Response](https://github.com/bohdaq/rust-web-server/tree/main/src/response)
- [Server](https://github.com/bohdaq/rust-web-server/tree/main/src/server)
- [Application](https://github.com/bohdaq/rust-web-server/tree/main/src/application)
