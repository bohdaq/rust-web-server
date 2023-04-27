[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > Server 

# Server 

Server module is designed to set up TcpListener, accept incoming connection, parse the request and pass the request to the user-defined Application.

User Application is responsible for processing the request. By processing request, it means generating corresponding response.


### Usage
First, you need to start a new server instance via calling [Server::setup]() method. Then you need to make an instance of your [Application]() and pass it to the [Server::run]() method.

#### Links
- [Request]()
- [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
- [Response]()
