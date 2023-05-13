[Read Me](https://github.com/bohdaq/rust-web-server/blob/main/README.md) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Application 

# Application 

Application module is designed to perform a specific set of actions based on the incoming request.

The logic for action is encapsulated as a Controller instance. Controller does two things: checks whether it is applicable for client request and, if it is applicable, performs the action.

Application design guideline is to define mutable Response instance, check each controller for match and apply action. In such a case, multiple actions can be chained, it allows a developer to perform some pre- or post-processing for the response.

### Usage
To make your application, define new struct (line [12](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L12)). As application is executed among different threads, it needs to implement `Copy` and `Clone` traits (line [11](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L11)). 

Additionally `New` (line [14](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L14)) and `Application` (line [20](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L20)) traits need to be implemented.

`New` trait returns new instance of a struct.

`Application` trait defines `execute` (line [21](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L21)) method which is called on an instance of an `App` by a server. It takes incoming request and connection info as parameters. The method produces a result containing either response or error message as a string.

Internal implementation for the `execute` method is done via creating mutable instance of a Response. The response has `501` status code and a list of default headers such as timestamp, vary, cors and client hints.

As response instance is mutable, the controller can change fields contained by the response. As an example, controller can set appropriate status code, reason phrase, add headers and set response body.



Previous topic | Current Topic | Next Topic
--- |---------------| ---
[Server](https://github.com/bohdaq/rust-web-server/tree/main/src/server) | Application          | [Controller](https://github.com/bohdaq/rust-web-server/tree/main/src/controller)
