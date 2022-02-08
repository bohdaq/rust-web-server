use crate::header::Header;
use regex::Regex;
use crate::constant::CONSTANTS;

pub struct Request {
    pub(crate) method: String,
    pub(crate) request_uri: String,
    pub(crate) http_version: String,
    pub(crate) headers: Vec<Header>,
}

impl Request {
    pub(crate) const METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX: &'static str = "(?P<method>\\w+)\\s(?P<request_uri>[./A-Za-z0-9]+)\\s(?P<http_version>[/.A-Za-z0-9]+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_request(request: Request) -> String {
        let status = [request.method, request.request_uri, request.http_version, CONSTANTS.NEW_LINE_SEPARATOR.to_string()].join(CONSTANTS.WHITESPACE);

        let mut headers = CONSTANTS.EMPTY_STRING.to_string();
        for header in request.headers {
            let mut header_string = CONSTANTS.EMPTY_STRING.to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(": ");
            header_string.push_str(&header.header_value);
            header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            headers.push_str(&header_string);
        }

        let request = format!(
            "{}{}{}",
            status,
            headers,
            CONSTANTS.NEW_LINE_SEPARATOR
        );

        println!("_____REQUEST______\n{}", request);

        request
    }

    pub(crate) fn parse_request(request: &String) ->  Request {
        println!("_____REQUEST______\n{}", request);

        let strings: Vec<&str> = request.split(CONSTANTS.NEW_LINE_SEPARATOR).collect();

        // parsing method request_uri and http_version
        let method_request_uri_http_version = strings[0].to_string();

        let re = Regex::new(Request::METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX).unwrap();
        let caps = re.captures(&method_request_uri_http_version).unwrap();


        let method = String::from(&caps["method"]);
        let request_uri = String::from(&caps["request_uri"]);
        let http_version = String::from(&caps["http_version"]);

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