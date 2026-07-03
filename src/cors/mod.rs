#[cfg(test)]
mod tests;

use crate::header::Header;
use crate::request::{METHOD, Request};
use crate::response::Error;
use crate::server_config::ServerConfig;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Cors {
    pub allow_all: bool,
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
    pub allow_credentials: bool,
    pub expose_headers: Vec<String>,
    pub max_age: String,
}

impl Cors {
    pub const MAX_AGE: &'static str = "86400";

    pub fn get_vary_header_value() -> String {
        Header::_ORIGIN.to_string()
    }

    pub fn allow_all(request: &Request) -> Result<Vec<Header>, Error> {
        let mut headers : Vec<Header> = vec![];
        let origin = request.get_header(Header::_ORIGIN.to_string());
        if origin.is_some() {
            let allow_origin = Header {
                name: Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
                value: origin.unwrap().value.to_string()
            };
            headers.push(allow_origin);

            let allow_credentials = Header {
                name: Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                value: "true".to_string()
            };
            headers.push(allow_credentials);

            let is_options = request.method == METHOD.options;
            if is_options {
                let method = request.get_header(Header::_ACCESS_CONTROL_REQUEST_METHOD.to_string());
                if method.is_some() {
                    let allow_method = Header {
                        name: Header::_ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                        value: method.unwrap().value.to_string()
                    };
                    headers.push(allow_method);
                }

                let access_control_request_headers = request.get_header(Header::_ACCESS_CONTROL_REQUEST_HEADERS.to_string());
                if access_control_request_headers.is_some() {
                    let request_headers = access_control_request_headers.unwrap();
                    let allow_headers = Header {
                        name: Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                        value: request_headers.value.to_lowercase(),
                    };
                    headers.push(allow_headers);

                    let expose_headers = Header {
                        name: Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                        value: request_headers.value.to_lowercase(),
                    };
                    headers.push(expose_headers);
                }

                let max_age = Header {
                    name: Header::_ACCESS_CONTROL_MAX_AGE.to_string(),
                    value: Cors::MAX_AGE.to_string()
                };
                headers.push(max_age);
            }

        }

        Ok(headers)
    }

    pub fn _process(request: &Request, cors: &Cors) -> Result<Vec<Header>, Error> {
        let mut headers : Vec<Header> = vec![];

        let allow_origins = cors.allow_origins.join(",");
        let boxed_origin = request.get_header(Header::_ORIGIN.to_string());

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
            name: Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
            value: origin_value
        };
        headers.push(allow_origin);

        if cors.allow_credentials {
            let allow_credentials = Header {
                name: Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                value: cors.allow_credentials.to_string()
            };
            headers.push(allow_credentials);
        }

        let is_options = request.method == METHOD.options;
        if is_options {
            let methods = cors.allow_methods.join(",");
            let allow_methods = Header {
                name: Header::_ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                value: methods
            };
            headers.push(allow_methods);

            let allow_headers_value = cors.allow_headers.join(",");
            let allow_headers = Header {
                name: Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                value: allow_headers_value.to_lowercase()
            };
            headers.push(allow_headers);

            let allow_expose_headers  = cors.expose_headers.join(",");
            let expose_headers = Header {
                name: Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                value: allow_expose_headers.to_lowercase()
            };
            headers.push(expose_headers);

            let max_age = Header {
                name: Header::_ACCESS_CONTROL_MAX_AGE.to_string(),
                value: cors.max_age.to_string()
            };
            headers.push(max_age);
        }

        Ok(headers)
    }

    /// Build CORS headers using the provided [`ServerConfig`].
    ///
    /// This is the primary implementation. [`get_headers`] is the legacy
    /// entry point that reads from environment variables; all new call-sites
    /// should prefer this version.
    pub fn get_headers_from_config(request: &Request, config: &ServerConfig) -> Vec<Header> {
        if config.cors_allow_all {
            return Cors::allow_all(request).unwrap_or_default();
        }
        Cors::process_using_config(request, config).unwrap_or_default()
    }

    fn process_using_config(request: &Request, config: &ServerConfig) -> Result<Vec<Header>, Error> {
        let mut headers: Vec<Header> = vec![];

        let boxed_origin = request.get_header(Header::_ORIGIN.to_string());
        if boxed_origin.is_none() {
            return Ok(headers);
        }
        let origin_value = boxed_origin.unwrap().value.clone();

        if !config.cors_allow_origins.contains(&origin_value) {
            return Ok(headers);
        }

        headers.push(Header {
            name: Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
            value: origin_value,
        });

        let credentials_str = &config.cors_allow_credentials;
        if credentials_str.eq_ignore_ascii_case("true") {
            headers.push(Header {
                name: Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                value: "true".to_string(),
            });
        }

        if request.method == METHOD.options {
            if !config.cors_allow_methods.is_empty() {
                headers.push(Header {
                    name: Header::_ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                    value: config.cors_allow_methods.clone(),
                });
            }
            if !config.cors_allow_headers.is_empty() {
                headers.push(Header {
                    name: Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                    value: config.cors_allow_headers.to_lowercase(),
                });
            }
            if !config.cors_expose_headers.is_empty() {
                headers.push(Header {
                    name: Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                    value: config.cors_expose_headers.to_lowercase(),
                });
            }
            if !config.cors_max_age.is_empty() {
                headers.push(Header {
                    name: Header::_ACCESS_CONTROL_MAX_AGE.to_string(),
                    value: config.cors_max_age.clone(),
                });
            }
        }

        Ok(headers)
    }

    /// Legacy entry point that reads CORS settings from environment variables.
    ///
    /// Prefer [`get_headers_from_config`] when a [`ServerConfig`] is available
    /// (e.g. inside [`App::execute`]). This variant is kept for call-sites that
    /// do not yet have an `App`-level config reference.
    pub fn process_using_default_config(request: &Request) -> Result<Vec<Header>, Error> {
        let config = ServerConfig::from_env();
        Self::process_using_config(request, &config)
    }

    /// Legacy entry point that reads CORS settings from environment variables.
    ///
    /// Prefer [`get_headers_from_config`] when a [`ServerConfig`] is available.
    pub fn get_headers(request: &Request) -> Vec<Header> {
        let config = ServerConfig::from_env();
        Cors::get_headers_from_config(request, &config)
    }
}