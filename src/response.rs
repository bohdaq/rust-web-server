use crate::header::Header;

pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: String,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) message_body: String
}

impl Response {
    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_response(status: String, contents: &String) -> String {
        let response = format!(
            "{}\r\nContent-Length: {}\r\n\r\n{}",
            status,
            contents.len(),
            contents
        );
        response
    }

    pub(crate) fn parse_response(response: String) -> Response {
        let strings: Vec<&str> = response.split("\r\n").collect();

        // parsing http_version, status_code and reason phrase
        let http_version_status_code_reason_phrase = strings[0].to_string();
        let split_http_version_status_code_reason_phrase: Vec<&str> = http_version_status_code_reason_phrase.split(" ").collect();

        let http_version = split_http_version_status_code_reason_phrase[0].to_string();
        let status_code = split_http_version_status_code_reason_phrase[1].to_string();
        let reason_phrase = split_http_version_status_code_reason_phrase[2].to_string();

        // parsing headers
        let mut headers = vec![];
        let mut headers_end_position = 999999;
        for (pos, e) in strings.iter().enumerate() {
            // stop when headers end
            if e.len() <= 1 {
                headers_end_position = pos;
                break;
            }

            // skip http_version, status_code and reason phrase
            if pos != 0  {
                let header_parts: Vec<&str> = e.split(": ").collect();

                let header = Header {
                    header_name: header_parts[0].to_string(),
                    header_value: header_parts[1].to_string()
                };

                headers.push(header);

            }
        }

        let mut message_body = "".to_string();
        // parsing message body
        for (pos, e) in strings.iter().enumerate() {
            // start when headers end
            if pos > headers_end_position {
                message_body.push_str(e);
            }
        }

        Response {
            http_version,
            status_code,
            reason_phrase,
            headers,
            message_body,
        }
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Response http version {} and status_code {} and reason_phrase {}", self.http_version, self.status_code, self.reason_phrase)
    }
}