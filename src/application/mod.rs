use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

#[cfg(test)]
mod example;

pub trait Application {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String>;
}