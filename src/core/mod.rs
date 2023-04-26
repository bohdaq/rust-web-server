use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

pub trait New {
    fn new() -> Self;
}

pub trait Application {
    fn execute(&mut self, request: Request, connection: ConnectionInfo) -> Result<Response, String>;
}