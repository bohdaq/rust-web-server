[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > Application 

# Application 

Application module is designed to perform specific set of actions based on the incoming request.

The logic for action is encapsulated as a Controller instance. Controller does two things: checks whether it is applicable for client request and, if it is applicable, performs the action.

Application design guideline is to define mutable Response instance, check each controller for match and apply action. In such a case multiple actions can be chained, it allows developer to perform some pre or post processing for the response.

### Usage
To make your application, define new struct (line [12](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L12)). As application is executed among different threads, it needs to implement `Copy` and `Clone` traits (line [11](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L11)). 

Additionally `New` (line [14](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L14)) and `Application` (line [20](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L20)) traits need to be implemented.

`New` trait returns new instance of a struct.

Application trait defines `execute` (line [21](https://github.com/bohdaq/rust-web-server/blob/6e7e1ed6219644468dcd1caac7f75ddf7d527ad9/src/application/example/mod.rs#L21)) method which is called on an instance of an `App` by a server. It takes incoming request and connection info as parameters. In the following example 




#### Links

