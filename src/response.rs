use crate::header::Header;
use regex::Regex;
use crate::constant::CONSTANTS;

pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: String,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) message_body: Vec<u8>
}

impl Response {
    pub(crate) const HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_response(response: Response) -> Vec<u8> {
        let status = [response.http_version, response.status_code, response.reason_phrase].join(CONSTANTS.WHITESPACE);

        let mut headers = CONSTANTS.NEW_LINE_SEPARATOR.to_string();
        for header in response.headers {
            let mut header_string = CONSTANTS.EMPTY_STRING.to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.header_value);
            header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            headers.push_str(&header_string);
        }

        let mut content_length_header_string = CONSTANTS.EMPTY_STRING.to_string();
        content_length_header_string.push_str("Content-Length");
        content_length_header_string.push_str(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR);
        content_length_header_string.push_str(response.message_body.len().to_string().as_str());
        content_length_header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
        headers.push_str(&content_length_header_string);

        let response_without_body = format!(
            "{}{}{}",
            status,
            headers,
            CONSTANTS.NEW_LINE_SEPARATOR,
        );

        println!("_____RESPONSE w/o body______\n{}", &response_without_body);

        let mut response :Vec<u8> = Vec::from([response_without_body.into_bytes(), response.message_body].concat());


        response
    }

    pub(crate) fn parse_response(response: Vec<u8>) -> Response {
        println!("Vec<u8> length: {}", response.len());

        let len : usize = response.len();
        let iteration_end_position : usize = len - 4;
        let mut last_new_line_position: usize = 0;

        for i in 0..iteration_end_position {
            let first_byte = response[i];
            let second_byte = response[i+1];
            let third_byte = response[i+2];
            let fourth_byte = response[i+3];

            let char_as_u8_4 = [first_byte, second_byte, third_byte, fourth_byte];
            let char_as_u32 = Response::as_u32_be(&char_as_u8_4);
            let char = char::from_u32(char_as_u32).unwrap();

            if char == '\n' {
                let string_as_bytes_u8 = response[last_new_line_position..i];
                let string = String::from(string_as_bytes_u8);

                println!("{}", string);
                println!("Last new line position: {}", last_new_line_position);
                println!("Current new line position: {}", i);

                if last_new_line_position == 0 {
                    let (http_version, status_code, reason_phrase) = Response::parse_http_version_status_code_reason_phrase_string(&string);
                    println!("http_version: {} status_code: {} reason_phrase: {}", http_version, status_code, reason_phrase);
                }

                if last_new_line_position != 0 {
                    if string.len() <= 1 {
                        println!("detected end of headers part");
                        break;
                    } else {
                        Response::parse_http_response_header_string(&string);
                    }

                }

                last_new_line_position = i;

            }

        }


        let strings: Vec<&str> = response.split(CONSTANTS.NEW_LINE_SEPARATOR).collect();

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
                let header_parts: Vec<&str> = e.split(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR).collect();

                let header = Header {
                    header_name: header_parts[0].to_string(),
                    header_value: header_parts[1].to_string()
                };

                headers.push(header);

            }
        }

        let mut message_body = CONSTANTS.EMPTY_STRING.to_string();
        // parsing message body
        for (pos, e) in strings.iter().enumerate() {
            // start when headers end
            if pos > headers_end_position {
                message_body.push_str(e);
            }
        }
        let u8_message_body = message_body.as_bytes();

        Response {
            http_version,
            status_code,
            reason_phrase,
            headers,
            message_body: Vec::from(u8_message_body),
        }
    }

    pub(crate)  fn parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> (String, String, String) {
        let re = Regex::new(Response::HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let http_version= String::from(&caps["http_version"]);
        let status_code = String::from(&caps["status_code"]);
        let reason_phrase = String::from(&caps["reason_phrase"]);

        return (http_version, status_code, reason_phrase)
    }

    pub(crate)  fn parse_http_response_header_string(header: &str) {

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
                let header_parts: Vec<&str> = e.split(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR).collect();

                let header = Header {
                    header_name: header_parts[0].to_string(),
                    header_value: header_parts[1].to_string()
                };

                headers.push(header);

            }
        }
    }

    pub(crate) fn as_u32_be(array: &[u8; 4]) -> u32 {
            ((array[0] as u32) << 24 )  |
            ((array[1] as u32) << 16)   |
            ((array[2] as u32) << 8)    |
            ((array[3] as u32) << 0)
    }
}