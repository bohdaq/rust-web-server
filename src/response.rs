use crate::header::Header;
use regex::Regex;
use crate::constant::CONSTANTS;

pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: String,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) message_body: String
}

impl Response {
    pub(crate) const HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_response(response: Response) -> String {
        let status = [response.http_version, response.status_code, response.reason_phrase].join(" ");

        let mut headers = CONSTANTS.NEW_LINE_SEPARATOR.to_string();
        for header in response.headers {
            let mut header_string = "".to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(": ");
            header_string.push_str(&header.header_value);
            header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            headers.push_str(&header_string);
        }

        let mut content_length_header_string = "".to_string();
        content_length_header_string.push_str("Content-Length");
        content_length_header_string.push_str(": ");
        content_length_header_string.push_str(response.message_body.len().to_string().as_str());
        content_length_header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
        headers.push_str(&content_length_header_string);

        let response = format!(
            "{}{}{}{}",
            status,
            headers,
            CONSTANTS.NEW_LINE_SEPARATOR,
            response.message_body,
        );

        println!("_____RESPONSE______\n{}", response);

        response
    }

    pub(crate) fn parse_response(response: String) -> Response {
        let strings: Vec<&str> = response.split(CONSTANTS.NEW_LINE_SEPARATOR).collect();

        // parsing http_version, status_code and reason phrase
        let http_version_status_code_reason_phrase = strings[0].to_string();

        let re = Regex::new(Response::HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let http_version= String::from(&caps["http_version"]);
        let status_code = String::from(&caps["status_code"]);
        let reason_phrase = String::from(&caps["reason_phrase"]);

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