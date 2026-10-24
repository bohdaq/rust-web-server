#[cfg(test)]
mod tests;
#[cfg(test)]
mod example;
pub mod path;

use std::collections::HashMap;
use url_build_parse::{build_url, parse_url, UrlComponents};
use url_search_params::{build_url_search_params, encode_uri_component, parse_url_search_params};
use url_search_params::decode_uri_component;

/// URL parsing, building, and percent-encoding utilities.
pub struct URL;

impl URL {
    pub fn percent_encode(component: &str) -> String {
        encode_uri_component(component)
    }

    pub fn percent_decode(component: &str) -> String {
        decode_uri_component(component)
    }

    pub fn build_query(params: HashMap<String, String>) -> String {
        build_url_search_params(params)
    }

    pub fn parse_query(component: &str) -> HashMap<String, String> {
        parse_url_search_params(component)
    }

    pub fn build(components: UrlComponents) -> Result<String, String> {
        build_url(components)
    }

    pub fn parse(url: &str) -> Result<UrlComponents, String> {
        parse_url(url)
    }

    /// Parse an origin-form request target (`/path?query`) — what
    /// `Request::request_uri` holds. Anything not starting with `/` is an
    /// error rather than being glued onto a placeholder authority, where a
    /// target like `:x` would become the port of `http://localhost:x`.
    pub fn parse_request_target(target: &str) -> Result<UrlComponents, String> {
        if !target.starts_with('/') {
            return Err(format!("not an origin-form request target: {:?}", target));
        }
        parse_url(&format!("http://localhost{}", target))
    }
}