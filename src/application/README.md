[Read Me](https://github.com/bohdaq/rust-web-server/tree/main) > Application 

# Application 

Application module is designed to perform specific set of actions based on the incoming request.

The logic for action is encapsulated as a Controller instance. Controller does two things: checks whether it is applicable for client request and, if it is applicable, performs the action.

Application design guideline is to define mutable Response instance, check each controller for match and apply action. In such a case multiple actions can be chained, it allows developer to perform some pre or post processing for the response.

### Usage



#### Links

