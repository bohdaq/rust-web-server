use crate::{Request, Response};
use crate::constant::{HTTPError, REQUEST_METHODS};
use crate::header::Header;

pub struct Cors {

}

impl Cors {
    pub(crate) const MAX_AGE: &'static str = "86400";

    pub(crate) fn allow_all(request: Request, mut response: Response) -> Result<(Request, Response), HTTPError> {
        let origin = request.get_header(Header::ORIGIN.to_string());
        if origin.is_some() {
            let allow_origin = Header {
                header_name: Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
                header_value: origin.unwrap().header_value.to_string()
            };
            response.headers.push(allow_origin);

            let allow_credentials = Header {
                header_name: Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(),
                header_value: "true".to_string()
            };
            response.headers.push(allow_credentials);

            let vary = Header {
                header_name: Header::VARY.to_string(),
                header_value: Header::ORIGIN.to_string()
            };
            response.headers.push(vary);


            let is_options = request.method == REQUEST_METHODS.OPTIONS;
            if is_options {
                let method = request.get_header(Header::ACCESS_CONTROL_REQUEST_METHOD.to_string());
                if method.is_some() {
                    let allow_method = Header {
                        header_name: Header::ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                        header_value: method.unwrap().header_value.to_string()
                    };
                    response.headers.push(allow_method);
                }

                let headers = request.get_header(Header::ACCESS_CONTROL_REQUEST_HEADERS.to_string());
                if headers.is_some() {
                    let request_headers = headers.unwrap();
                    let allow_headers = Header {
                        header_name: Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                        header_value: request_headers.header_value.to_string()
                    };
                    response.headers.push(allow_headers);

                    let expose_headers = Header {
                        header_name: Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string(),
                        header_value: request_headers.header_value.to_string()
                    };
                    response.headers.push(expose_headers);
                }

                let max_age = Header {
                    header_name: Header::ACCESS_CONTROL_MAX_AGE.to_string(),
                    header_value: Cors::MAX_AGE.to_string()
                };
                response.headers.push(max_age);
            }

        }

        Ok((request, response))
    }
}