#[cfg(test)]
mod tests;

use url_search_params::encode_uri_component;
use url_search_params::decode_uri_component;

pub struct URL;

impl URL {
    pub fn percent_encode(component: &str) -> String {
        encode_uri_component(component)
    }

    pub fn percent_decode(component: &str) -> String {
        decode_uri_component(component)
    }
}