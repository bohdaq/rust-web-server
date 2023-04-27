use crate::app::App;
use crate::core::New;
use crate::server::Server;

// not a test because Server::run runs infinite loop to listen for incoming connections
// it means test would never finish
fn _example() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
    }


    let (listener, pool) = new_server.unwrap();
    let app = App::new();


    // server listens for incoming connections and executes your app's logic via thread pool
    Server::run(listener, pool, app);
}