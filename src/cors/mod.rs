#[cfg(test)]
mod tests;

use std::env;
use crate::header::Header;

use crate::entry_point::Config;
use crate::request::{METHOD, Request};
use crate::response::{Error};

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

    pub fn process_using_default_config(request: &Request) -> Result<Vec<Header>, Error> {
        let mut headers : Vec<Header> = vec![];
        let boxed_allow_origins = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
        let mut allow_origins: String = "".to_string();
        if boxed_allow_origins.is_err() {
            eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
        } else {
            allow_origins = boxed_allow_origins.unwrap();
        }

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

        let boxed_is_allow_credentials = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
        if boxed_is_allow_credentials.is_err() {
            eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
        } else {
            let boxed_parse = boxed_is_allow_credentials.unwrap().parse::<bool>();
            if boxed_parse.is_err() {
                eprintln!("unable to parse as boolean {} environment variable. Possible values are true or false", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
            } else {
                let is_allow_credentials : bool = boxed_parse.unwrap();
                if is_allow_credentials {
                    let allow_credentials = Header {
                        name: Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                        value: is_allow_credentials.to_string()
                    };
                    headers.push(allow_credentials);
                }
            }
        }


        let is_options = request.method == METHOD.options;
        if is_options {
            let boxed_methods = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS);
            if boxed_methods.is_err() {
                eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_ALLOW_METHODS);
            } else {
                let methods = boxed_methods.unwrap();
                let allow_methods = Header {
                    name: Header::_ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                    value: methods
                };
                headers.push(allow_methods);
            }


            let boxed_allow_headers_env_variable = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
            if boxed_allow_headers_env_variable.is_err() {
                eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
            } else {
                let allow_headers_env_variable = boxed_allow_headers_env_variable.unwrap();
                let allow_headers = Header {
                    name: Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                    value: allow_headers_env_variable.to_lowercase()
                };
                headers.push(allow_headers);
            }


            let boxed_allow_expose_headers = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
            if boxed_allow_expose_headers.is_err() {
                eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
            } else {
                let allow_expose_headers  = boxed_allow_expose_headers.unwrap();
                let expose_headers = Header {
                    name: Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                    value: allow_expose_headers.to_lowercase()
                };
                headers.push(expose_headers);
            }


            let boxed_max_age_value = env::var(Config::RWS_CONFIG_CORS_MAX_AGE);
            if boxed_max_age_value.is_err() {
                eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_MAX_AGE);
            } else {
                let max_age_value  = boxed_max_age_value.unwrap();
                let max_age = Header {
                    name: Header::_ACCESS_CONTROL_MAX_AGE.to_string(),
                    value: max_age_value
                };
                headers.push(max_age);
            }

        }


        Ok(headers)
    }

    pub fn get_headers(request: &Request) -> Vec<Header> {

        let boxed_rws_config_cors_allow_all = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL);
        if boxed_rws_config_cors_allow_all.is_err() {
            eprintln!("unable to read {} environment variable", Config::RWS_CONFIG_CORS_ALLOW_ALL);
            let boxed_cors_header_list = Cors::allow_all(&request);
            if boxed_cors_header_list.is_err() {
                eprintln!("unable to get Cors::allow_all headers {}", boxed_cors_header_list.err().unwrap().message);
            } else {
                return boxed_cors_header_list.unwrap()
            }
        } else {
            let boxed_parse = boxed_rws_config_cors_allow_all.unwrap().parse::<bool>();
            if boxed_parse.is_err() {
                eprintln!("unable to parse as boolean {} environment variable. Possible values are true or false", Config::RWS_CONFIG_CORS_ALLOW_ALL);
            } else {
                let is_cors_set_to_allow_all_requests = boxed_parse.unwrap();
                if !is_cors_set_to_allow_all_requests {
                    let boxed_cors_header_list = Cors::process_using_default_config(&request);
                    if boxed_cors_header_list.is_err() {
                        eprintln!("unable to get Cors::process_using_default_config headers {}", boxed_cors_header_list.err().unwrap().message);
                    } else {
                        return boxed_cors_header_list.unwrap()
                    }
                }
            }
        }


        let boxed_cors_header_list = Cors::allow_all(&request);
        if boxed_cors_header_list.is_err() {
            eprintln!("unable to get Cors::allow_all headers {}", boxed_cors_header_list.err().unwrap().message);
            vec![]
        } else {
            return boxed_cors_header_list.unwrap()
        }
    }
}