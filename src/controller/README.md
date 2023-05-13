[Read Me](https://github.com/bohdaq/rust-web-server/blob/main/README.md) > [Documentation](https://github.com/bohdaq/rust-web-server/tree/main/src/README.md) > Controller 

# Controller 

Controller module is designed to perform an action for a specific request.

Controller does two things: checks whether it is applicable for request and, if it is applicable, performs the action.

Let's take `IndexController` as an example.

### Usage
[IndexController](https://github.com/bohdaq/rust-web-server/blob/main/src/controller/example/mod.rs) responsibility is to prepare http response containing `index.html` page. First you define struct (line [10](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L10)).
 
As we are going to use the name of index file, we defined the `INDEX_FILEPATH` constant inside `IndexController` implementation (line [13](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L13)).

#### Matching the request

To make our struct a `Controller`, we need to implement Controller trait (line [16](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L16)). Matching request starts with a slash and has GET method (line [18](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L18)). 

Matching can be done using any request field and connection info supplied to a function as parameters. Connection info contains ip and port for a client and server. Request contains http version, method, uri, headers and body.

As you can see `is_matching` method does return a boolean. It is done without explicitly using `return` keyword (line [18](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L18)), as at the end of line there is no a semicolon, which means the line returns result of evaluation (boolean) to the outer context (`is_matching` function). Such line is called `expression`. If you add a semicolon at the end of the line, it will not return a result of evaluation to the outer context, such line is called `statement`. Statement always returns empty result tuple  `()`, called [unit](https://doc.rust-lang.org/std/primitive.unit.html).

#### Preparing the response
The result of a `process` method is Response instance (line [59](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L59)). 

In case of an error, a `process` method will return a response (line [35](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L35) - [45]((line [59](https://github.com/bohdaq/rust-web-server/blob/149d608841ad77b69e2147143928220d29195988/src/controller/example/mod.rs#L45))). It may be modified content or error message. 

The server returns the response to the client
and doesn't have the ability to process errors happening in the user application.

So it's up to application developer to make sure errors are handled properly and corresponding responses are sent.

On high level `IndexController` checks if there is a file named `index.html` in server's root folder (line [26](https://github.com/bohdaq/rust-web-server/blob/348d1051e7b04ec0eb254d8d62864f0d23bf6ae2/src/controller/example/mod.rs#L26)). If such file exists, the controller will try to open and read the contents of the file, and return it as http response (line [27](https://github.com/bohdaq/rust-web-server/blob/348d1051e7b04ec0eb254d8d62864f0d23bf6ae2/src/controller/example/mod.rs#L27) - [45](https://github.com/bohdaq/rust-web-server/blob/348d1051e7b04ec0eb254d8d62864f0d23bf6ae2/src/controller/example/mod.rs#L45)).

If there is no file `index.html` inside server root folder, controller will put the contents of the default [index.html](https://github.com/bohdaq/rust-web-server/blob/main/src/controller/example/index.html) (line [48](https://github.com/bohdaq/rust-web-server/blob/348d1051e7b04ec0eb254d8d62864f0d23bf6ae2/src/controller/example/mod.rs#L48) - [55](https://github.com/bohdaq/rust-web-server/blob/348d1051e7b04ec0eb254d8d62864f0d23bf6ae2/src/controller/example/mod.rs#L55)).

Default `index.html` file is shipped as the part of the binary (line [48](https://github.com/bohdaq/rust-web-server/blob/main/src/controller/example/index.html)) via `include_bytes!` macro.


Previous topic | Current Topic | Next Topic
--- |---------------| ---
[Application](https://github.com/bohdaq/rust-web-server/tree/main/src/application) | Controller          | [Body](https://github.com/bohdaq/rust-web-server/tree/main/src/body)
