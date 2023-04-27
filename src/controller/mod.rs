#[cfg(test)]
mod example;

use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

pub trait Controller {
    fn is_matching(request: &Request, connection: &ConnectionInfo) -> bool;
    fn process(request: &Request, response: Response, connection: &ConnectionInfo) -> Response;
}