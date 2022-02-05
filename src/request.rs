use crate::header::Header;

pub struct Request {
    pub(crate) method: String,
    pub(crate) request_uri: String,
    pub(crate) http_version: String,
    pub(crate) headers: Vec<Header>,
}

impl Request {
    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_request(request: Request) -> String {
        let status = [request.method, request.request_uri, request.http_version, "\r\n".to_string()].join(" ");

        let mut headers = "".to_string();
        for header in request.headers {
            let mut header_string = "".to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(": ");
            header_string.push_str(&header.header_value);
            header_string.push_str("\r\n");
            headers.push_str(&header_string);
        }

        let request = format!(
            "{}{}\r\n",
            status,
            headers,
        );

        request
    }

    pub(crate) fn parse_request(request: &String) ->  Request {
        let strings: Vec<&str> = request.split("\r\n").collect();

        // parsing method request_uri and http_version
        let method_request_uri_http_version = strings[0].to_string();
        let split_method_request_uri_http_version: Vec<&str> = method_request_uri_http_version.split(" ").collect();

        let method = split_method_request_uri_http_version[0];
        let request_uri = split_method_request_uri_http_version[1];
        let http_version = split_method_request_uri_http_version[2];


        let mut headers = vec![];
        // parsing headers
        for (pos, e) in strings.iter().enumerate() {
            // stop when headers end
            if e.len() <= 1 {
                break;
            }

            // skip method_request_uri_http_version
            if pos != 0  {
                let header_parts: Vec<&str> = e.split(": ").collect();

                let header = Header {
                    header_name: header_parts[0].to_string(),
                    header_value: header_parts[1].to_string()
                };

                headers.push(header);

            }
        }

        Request {
            method: method.to_string(),
            request_uri: request_uri.to_string(),
            http_version: http_version.to_string(),
            headers,
        }
    }

}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}