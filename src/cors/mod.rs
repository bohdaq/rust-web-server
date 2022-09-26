#[cfg(test)]
mod tests;

use std::env;
use crate::header::Header;

use serde::{Serialize, Deserialize};
use crate::entry_point::Config;
use crate::request::{METHOD, Request};
use crate::response::{Error};

#[derive(Debug, Serialize, Deserialize)]
pub struct Cors {
    pub(crate) allow_all: bool,
    pub(crate) allow_origins: Vec<String>,
    pub(crate) allow_methods: Vec<String>,
    pub(crate) allow_headers: Vec<String>,
    pub(crate) allow_credentials: bool,
    pub(crate) expose_headers: Vec<String>,
    pub(crate) max_age: String,
}

impl Cors {
    pub(crate) const MAX_AGE: &'static str = "86400";

    pub(crate) fn allow_all(request: &Request) -> Result<Vec<Header>, Error> {
        let mut headers : Vec<Header> = vec![];
        let origin = request.get_header(Header::ORIGIN.to_string());
        if origin.is_some() {
            let allow_origin = Header {
                name: Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
                value: origin.unwrap().value.to_string()
            };
            headers.push(allow_origin);

            let allow_credentials = Header {
                name: Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                value: "true".to_string()
            };
            headers.push(allow_credentials);

            let vary = Header {
                name: Header::VARY.to_string(),
                value: Header::ORIGIN.to_string()
            };
            headers.push(vary);


            let is_options = request.method == METHOD.options;
            if is_options {
                let method = request.get_header(Header::ACCESS_CONTROL_REQUEST_METHOD.to_string());
                if method.is_some() {
                    let allow_method = Header {
                        name: Header::ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                        value: method.unwrap().value.to_string()
                    };
                    headers.push(allow_method);
                }

                let access_control_request_headers = request.get_header(Header::ACCESS_CONTROL_REQUEST_HEADERS.to_string());
                if access_control_request_headers.is_some() {
                    let request_headers = access_control_request_headers.unwrap();
                    let allow_headers = Header {
                        name: Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                        value: request_headers.value.to_lowercase(),
                    };
                    headers.push(allow_headers);

                    let expose_headers = Header {
                        name: Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                        value: request_headers.value.to_lowercase(),
                    };
                    headers.push(expose_headers);
                }

                let max_age = Header {
                    name: Header::ACCESS_CONTROL_MAX_AGE.to_string(),
                    value: Cors::MAX_AGE.to_string()
                };
                headers.push(max_age);
            }

        }

        Ok(headers)
    }

    pub(crate) fn _process(request: &Request, cors: &Cors) -> Result<Vec<Header>, Error> {
        let mut headers : Vec<Header> = vec![];

        let allow_origins = cors.allow_origins.join(",");
        let boxed_origin = request.get_header(Header::ORIGIN.to_string());

        if boxed_origin.is_none() {
            return Ok(headers)
        }

        let origin = boxed_origin.unwrap();
        let origin_value = format!("{}", origin.value);

        let is_valid_origin = allow_origins.contains(&origin_value);
        if !is_valid_origin {
            return Ok(headers)
        }

        let allow_origin = Header {
            name: Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
            value: origin_value
        };
        headers.push(allow_origin);

        if cors.allow_credentials {
            let allow_credentials = Header {
                name: Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                value: cors.allow_credentials.to_string()
            };
            headers.push(allow_credentials);
        }

        let vary = Header {
            name: Header::VARY.to_string(),
            value: Header::ORIGIN.to_string(),
        };
        headers.push(vary);

        let is_options = request.method == METHOD.options;
        if is_options {
            let methods = cors.allow_methods.join(",");
            let allow_methods = Header {
                name: Header::ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                value: methods
            };
            headers.push(allow_methods);

            let allow_headers_value = cors.allow_headers.join(",");
            let allow_headers = Header {
                name: Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                value: allow_headers_value.to_lowercase()
            };
            headers.push(allow_headers);

            let allow_expose_headers  = cors.expose_headers.join(",");
            let expose_headers = Header {
                name: Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                value: allow_expose_headers.to_lowercase()
            };
            headers.push(expose_headers);

            let max_age = Header {
                name: Header::ACCESS_CONTROL_MAX_AGE.to_string(),
                value: cors.max_age.to_string()
            };
            headers.push(max_age);
        }

        Ok(headers)
    }

    pub(crate) fn process_using_default_config(request: &Request) -> Result<Vec<Header>, Error> {
        let mut headers : Vec<Header> = vec![];
        let allow_origins : String = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS).unwrap();

        let boxed_origin = request.get_header(Header::ORIGIN.to_string());

        if boxed_origin.is_none() {
            return Ok(headers)
        }

        let origin = boxed_origin.unwrap();
        let origin_value = format!("{}", origin.value);

        let is_valid_origin = allow_origins.contains(&origin_value);
        if !is_valid_origin {
            return Ok(headers)
        }

        let allow_origin = Header {
            name: Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
            value: origin_value
        };
        headers.push(allow_origin);

        let is_allow_credentials : bool = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS).unwrap().parse().unwrap();
        if is_allow_credentials {
            let allow_credentials = Header {
                name: Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                value: is_allow_credentials.to_string()
            };
            headers.push(allow_credentials);
        }

        let vary = Header {
            name: Header::VARY.to_string(),
            value: Header::ORIGIN.to_string(),
        };
        headers.push(vary);

        let is_options = request.method == METHOD.options;
        if is_options {
            let methods = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS).unwrap();
            let allow_methods = Header {
                name: Header::ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                value: methods
            };
            headers.push(allow_methods);

            let allow_headers_env_variable = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS).unwrap();
            let allow_headers = Header {
                name: Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                value: allow_headers_env_variable.to_lowercase()
            };
            headers.push(allow_headers);

            let allow_expose_headers  = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS).unwrap();
            let expose_headers = Header {
                name: Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                value: allow_expose_headers.to_lowercase()
            };
            headers.push(expose_headers);

            let max_age_value  = env::var(Config::RWS_CONFIG_CORS_MAX_AGE).unwrap();
            let max_age = Header {
                name: Header::ACCESS_CONTROL_MAX_AGE.to_string(),
                value: max_age_value
            };
            headers.push(max_age);
        }


        Ok(headers)
    }
}