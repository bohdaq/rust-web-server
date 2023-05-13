[Read Me](https://github.com/bohdaq/rust-web-server/blob/main/README.md) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md)  > Server 

# Server 

Server module is designed to set up TcpListener, accept incoming connection, parse the request and pass the request to the user-defined Application.

User Application is responsible for processing the request. By processing request, it means generating corresponding response.


### Usage
First, you need to start a new server instance via calling [Server::setup](https://github.com/bohdaq/rust-web-server/blob/main/src/server/example/mod.rs#L8) method. Then you need to make an instance of your [Application](https://github.com/bohdaq/rust-web-server/blob/main/src/server/example/mod.rs#L15) and pass it to the [Server::run](https://github.com/bohdaq/rust-web-server/blob/main/src/server/example/mod.rs#L19) method.

Previous topic | Current Topic | Next Topic
--- |---------------| ---
[Response](https://github.com/bohdaq/rust-web-server/tree/main/src/response) | Server          | [Application](https://github.com/bohdaq/rust-web-server/tree/main/src/application)
