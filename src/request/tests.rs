use std::collections::HashMap;
use file_ext::FileExt;
use crate::body::form_urlencoded::FormUrlEncoded;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::symbol::SYMBOL;

#[test]
fn method() {
    assert_eq!(METHOD.get, "GET");
    assert_eq!(METHOD.head, "HEAD");
    assert_eq!(METHOD.post, "POST");
    assert_eq!(METHOD.put, "PUT");
    assert_eq!(METHOD.delete, "DELETE");
    assert_eq!(METHOD.connect, "CONNECT");
    assert_eq!(METHOD.options, "OPTIONS");
    assert_eq!(METHOD.trace, "TRACE");
    assert_eq!(METHOD.patch, "PATCH");
}

#[test]
fn method_and_request_uri_and_http_version_regex() {
    let method_request_uri_version = "GET / HTTP/1.1";
    let (method, request_uri, http_version) = Request::parse_method_and_request_uri_and_http_version_string(method_request_uri_version).unwrap();

    assert_eq!(VERSION.http_1_1, http_version.to_uppercase());
    assert_eq!(METHOD.get, method.to_uppercase());
    assert_eq!(SYMBOL.slash, &request_uri);


    let method_request_uri_version = "GET /draho-brat_pt2/drahobrat_pt2_ver2.mp4 HTTP/1.1";
    let (method, request_uri, http_version) = Request::parse_method_and_request_uri_and_http_version_string(method_request_uri_version).unwrap();


    assert_eq!(VERSION.http_1_1, http_version.to_uppercase());
    assert_eq!(METHOD.get, method.to_uppercase());
    assert_eq!("/draho-brat_pt2/drahobrat_pt2_ver2.mp4", &request_uri);

}

#[test]
fn test_request_ok() {
    let method = METHOD.get;
    let request_uri = SYMBOL.slash;
    let http_version = VERSION.http_1_1;

    let request_data = [method, request_uri, http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let request = Request::parse_request(raw_request.as_bytes()).unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_ok_with_special_characters() {
    let method = METHOD.get;
    let special_characters = "_:;.,/\"'?!(){}[]@<>=-+*#$&`|~^%";
    let request_uri = [SYMBOL.slash, special_characters].join("");
    let http_version = VERSION.http_1_1;


    let request_data = [method, request_uri.as_str(), http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let request = Request::parse_request(raw_request.as_bytes()).unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_ok_with_ukrainian_characters() {
    let method = METHOD.get;
    let ukrainian_characters = "АаБбВвГгҐґДдЕеЄєЖжЗзИиІіЇїЙйКкЛлМмНнОоПпРрСсТтУуФфХхЦцЧчШшЩщЬьЮюЯя";
    let request_uri = [SYMBOL.slash, ukrainian_characters].join("");
    let http_version = VERSION.http_1_1;


    let request_data = [method, request_uri.as_str(), http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let request = Request::parse_request(raw_request.as_bytes()).unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_not_ok() {
    let method = METHOD.get;
    let request_uri = [SYMBOL.slash, SYMBOL.whitespace, SYMBOL.hyphen].join("");
    let http_version = VERSION.http_1_1;

    let request_data = [method, request_uri.as_str(), http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let boxed_request = Request::parse_request(raw_request.as_bytes());
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_not_ok_empty_request() {
    let boxed_request = Request::parse_request(b"");
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_not_ok_dummy_not_valid_request() {
    let dummy_request = "some dummy not valid request";
    let boxed_request = Request::parse_request(dummy_request.as_bytes());
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_not_ok_zeros_request() {
    let dummy_request = b"00000000";
    let boxed_request = Request::parse_request(dummy_request);
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_empty_request_uri() {
    let method = METHOD.get;
    let request_uri = "";
    let http_version = VERSION.http_1_1;

    let request_line = [method, request_uri, http_version].join(" ");

    // RFC 9112 §3: the request-target is not optional.
    let boxed_request = Request::parse_request(request_line.as_bytes());
    assert!(boxed_request.is_err());
}

#[test]
fn test_request_lowercase() {
    let method = METHOD.get.to_lowercase();
    let request_uri = "/path";
    let http_version = VERSION.http_1_1.to_lowercase();

    let request_line = [method.to_string(), request_uri.to_string(), http_version.to_string()].join(" ");

    let boxed_request = Request::parse_request(request_line.as_bytes());

    let request = boxed_request.unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_lowercase_not_valid_utf_8() {
    let mut non_utf8_char: Vec<u8> = vec![255];

    let method = METHOD.get.to_lowercase();
    let request_uri = "/path";
    let http_version = VERSION.http_1_1.to_lowercase();

    let mut request_vec : Vec<u8> = vec![];
    request_vec.append(&mut method.as_bytes().to_vec());
    request_vec.append(&mut request_uri.as_bytes().to_vec());
    request_vec.append(&mut non_utf8_char);
    request_vec.append(&mut http_version.as_bytes().to_vec());

    let boxed_request = Request::parse_request(&request_vec);

    assert_eq!(true, boxed_request.is_err());
    assert_eq!("invalid utf-8 sequence of 1 bytes from index 8", boxed_request.err().unwrap());
}

#[test]
fn test_request_randomcase() {
    let method = "GeT";
    let request_uri = "/path";
    let http_version = "HtTP/1.1";

    let request_line = [method.to_string(), request_uri.to_string(), http_version.to_string()].join(" ");

    let boxed_request = Request::parse_request(request_line.as_bytes());

    let request = boxed_request.unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn text_plain_content_type() {

    //let raw_request = String::from_utf8(Vec::from(request)).unwrap();
    //println!("\n\n______{}______\n\n", raw_request);

    let raw_request_1 = format!("POST /file-upload HTTP/1.1{}", SYMBOL.new_line_carriage_return);
    let raw_request_2 = format!("Content-Type: text/plain{}", SYMBOL.new_line_carriage_return);
    let raw_request_3 = SYMBOL.new_line_carriage_return.to_string();
    let raw_request_4 = format!("some-prop 1234{}", SYMBOL.new_line_carriage_return);

    let raw_request = [
        raw_request_1,
        raw_request_2,
        raw_request_3,
        raw_request_4,
    ].join(SYMBOL.empty_string);

    let boxed_request = Request::parse_request(raw_request.as_bytes());
    assert!(boxed_request.is_ok());

    let request = boxed_request.unwrap();

    let uri = "/file-upload";
    let method = "POST";
    let http_version = "HTTP/1.1";
    let content_type = "text/plain";

    assert_eq!(uri, request.request_uri);
    assert_eq!(method, request.method);
    assert_eq!(http_version, request.http_version);

    let content_type_header = request.get_header("Content-Type".to_string()).unwrap();
    assert_eq!(content_type_header.value, content_type);

}


#[test]
fn multipart_form_data_content_type() {

    //let raw_request = String::from_utf8(Vec::from(request)).unwrap();
    //println!("\n\n______{}______\n\n", raw_request);
    let boundary = "------hdfkjshdfkljashdgkh";


    let new_line = SYMBOL.new_line_carriage_return.to_string();


    let payload = "123".to_string();
    let key = "some";
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"{}", key, SYMBOL.new_line_carriage_return);
    let raw_payload_key_value = [
        payload_boundary,
        content_disposition,
        new_line.to_string(),
        payload,
        new_line.to_string(),
    ].join(SYMBOL.empty_string);


    let payload = "45678".to_string();
    let key = "key";
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"{}", key, SYMBOL.new_line_carriage_return);
    let raw_payload_another_key_value = [
        payload_boundary,
        content_disposition,
        new_line.to_string(),
        payload,
        new_line.to_string(),
    ].join(SYMBOL.empty_string);

    let filename = "rws.config.toml";
    let path = FileExt::build_path(&["src", "test", filename]);
    let boxed_payload = FileExt::read_file(&path);
    assert!(boxed_payload.is_ok());

    let payload = boxed_payload.unwrap();
    let key = "fileupload";
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
    let raw_payload_file = [
        payload_boundary,
        content_disposition,
        new_line.to_string(),
        String::from_utf8(payload).unwrap(), // payload is not escaped, text file used for test
        new_line.to_string(),
    ].join(SYMBOL.empty_string);

    let raw_payload = [
        raw_payload_key_value,
        raw_payload_another_key_value,
        raw_payload_file,
        boundary.to_string(),
    ].join(SYMBOL.empty_string);

    let uri = "/file-upload";
    let method = "POST";
    let http_version = "HTTP/1.1";
    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let head = format!("{} {} {} {}", method, uri, http_version, SYMBOL.new_line_carriage_return);
    let multipart_form_data_content_type = format!("Content-Type: {}{}", content_type, SYMBOL.new_line_carriage_return);
    let body = raw_payload.to_string();

    let raw_request = [
        head,
        multipart_form_data_content_type,
        new_line.to_string(),
        body,
    ].join(SYMBOL.empty_string);

    let boxed_request = Request::parse_request(raw_request.as_bytes());

    assert!(boxed_request.is_ok());

    let request = boxed_request.unwrap();
    assert_eq!(uri, request.request_uri);
    assert_eq!(method, request.method);
    assert_eq!(http_version, request.http_version);

    let content_type_header = request.get_header("Content-Type".to_string()).unwrap();
    assert_eq!(content_type_header.value, content_type);

    assert_eq!(raw_payload.as_bytes(), request.body);
}


#[test]
fn form_urlencoded_content_type() {

    //let raw_request = String::from_utf8(Vec::from(request)).unwrap();
    //println!("\n\n______{}______\n\n", raw_request);

    let head = format!("POST /form-upload HTTP/1.1{}", SYMBOL.new_line_carriage_return);
    let form_urlencoded_content_type = format!("Content-Type: application/x-www-form-urlencoded{}", SYMBOL.new_line_carriage_return);
    let new_line = SYMBOL.new_line_carriage_return.to_string();
    let body = format!("some=1234&key=5678");

    let raw_request = [
        head,
        form_urlencoded_content_type,
        new_line,
        body.to_string(),
    ].join(SYMBOL.empty_string);

    let boxed_request = Request::parse_request(raw_request.as_bytes());
    assert!(boxed_request.is_ok());

    let request = boxed_request.unwrap();

    let uri = "/form-upload";
    let method = "POST";
    let http_version = "HTTP/1.1";
    let content_type = "application/x-www-form-urlencoded";

    assert_eq!(uri, request.request_uri);
    assert_eq!(method, request.method);
    assert_eq!(http_version, request.http_version);

    let content_type_header = request.get_header("Content-Type".to_string()).unwrap();
    assert_eq!(content_type_header.value, content_type);

    assert_eq!(body.as_bytes(), request.body);

    let boxed_parse = FormUrlEncoded::parse(request.body.to_vec());
    let form = boxed_parse.unwrap();

    assert_eq!(form.get("key").unwrap(), "5678");
    assert_eq!(form.get("some").unwrap(), "1234");

}

#[test]
fn form_urlencoded_content_type_ends_with_new_line() {

    //let raw_request = String::from_utf8(Vec::from(request)).unwrap();
    //println!("\n\n______{}______\n\n", raw_request);

    let head = format!("POST /form-upload HTTP/1.1{}", SYMBOL.new_line_carriage_return);
    let form_urlencoded_content_type = format!("Content-Type: application/x-www-form-urlencoded{}", SYMBOL.new_line_carriage_return);
    let new_line = SYMBOL.new_line_carriage_return.to_string();
    let body = format!("some=1234&key=5678{}", SYMBOL.new_line_carriage_return);

    let raw_request = [
        head,
        form_urlencoded_content_type,
        new_line,
        body.to_string(),
    ].join(SYMBOL.empty_string);

    let boxed_request = Request::parse_request(raw_request.as_bytes());
    assert!(boxed_request.is_ok());

    let request = boxed_request.unwrap();

    let uri = "/form-upload";
    let method = "POST";
    let http_version = "HTTP/1.1";
    let content_type = "application/x-www-form-urlencoded";

    assert_eq!(uri, request.request_uri);
    assert_eq!(method, request.method);
    assert_eq!(http_version, request.http_version);

    let content_type_header = request.get_header("Content-Type".to_string()).unwrap();
    assert_eq!(content_type_header.value, content_type);

    assert_eq!(body.as_bytes(), request.body);

    let boxed_parse = FormUrlEncoded::parse(request.body.to_vec());
    let form = boxed_parse.unwrap();

    assert_eq!(form.get("key").unwrap(), "5678");
    assert_eq!(form.get("some").unwrap(), "1234");

}


#[test]
fn get_port() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "port.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_port : Result<Option<i128>, String> = request.get_port();
    if boxed_port.is_err() {
        // handle error
    }

    let port_option : Option<i128> = boxed_port.unwrap();
    if port_option.is_none() {
        // handle port absence
    }

    let port : i128 = port_option.unwrap();
    assert_eq!(7888, port);

}

#[test]
fn get_port_no_port() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "no-port.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_port : Result<Option<i128>, String> = request.get_port();
    if boxed_port.is_err() {
        // handle error
    }

    let port_option : Option<i128> = boxed_port.unwrap();
    assert!(port_option.is_none())


}

#[test]
fn get_port_no_host() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "no-host.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_port : Result<Option<i128>, String> = request.get_port();
    if boxed_port.is_err() {
        // handle error
    }

    let port_option : Option<i128> = boxed_port.unwrap();
    assert!(port_option.is_none())


}

#[test]
fn get_port_empty_host() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "empty-host.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_port : Result<Option<i128>, String> = request.get_port();
    if boxed_port.is_err() {
        // handle error
    }

    let port_option : Option<i128> = boxed_port.unwrap();
    assert!(port_option.is_none())


}


#[test]
fn get_domain_no_host() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "no-host.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_domain: Result<Option<String>, String> = request.get_domain();
    if boxed_domain.is_err() {
        // handle error
    }

    let domain_option: Option<String> = boxed_domain.unwrap();
    assert!(domain_option.is_none())


}

#[test]
fn get_domain_empty_host() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "empty-host.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_domain: Result<Option<String>, String> = request.get_domain();
    if boxed_domain.is_err() {
        // handle error
    }

    let domain_option: Option<String> = boxed_domain.unwrap();
    assert!(domain_option.is_none())


}

#[test]
fn get_domain_no_port() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "no-port.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_domain: Result<Option<String>, String> = request.get_domain();
    if boxed_domain.is_err() {
        // handle error
    }

    let domain_option: Option<String> = boxed_domain.unwrap();
    assert_eq!("127.0.0.1", domain_option.unwrap());


}

#[test]
fn get_domain_port_present() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "port.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_domain: Result<Option<String>, String> = request.get_domain();
    if boxed_domain.is_err() {
        // handle error
    }

    let domain_option: Option<String> = boxed_domain.unwrap();
    assert_eq!("127.0.0.1", domain_option.unwrap());


}

#[test]
fn get_query_no_query() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "no-query.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_domain: Result<Option<HashMap<String, String>>, String> = request.get_query();
    if boxed_domain.is_err() {
        // handle error
    }

    let domain_option: Option<HashMap<String, String>> = boxed_domain.unwrap();
    assert!(domain_option.is_none());

}

#[test]
fn get_query() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "query.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_domain: Result<Option<HashMap<String, String>>, String> = request.get_query();
    if boxed_domain.is_err() {
        // handle error
    }

    let domain_option: Option<HashMap<String, String>> = boxed_domain.unwrap();
    assert!(domain_option.is_some());

}

#[test]
fn get_path_no_path() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "no-path.query.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    // "GET ?some=1234&key=5678 HTTP/1.1" — a target without a leading "/"
    // isn't origin-form (RFC 9112 §3.2.1), so it's rejected outright.
    assert!(boxed_request.is_err());
}

#[test]
fn get_path() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "path.query.request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    // convert byte array to request
    let boxed_request = Request::parse(request_file_as_bytes.as_ref());
    if boxed_request.is_err() {
        let _error_message = boxed_request.as_ref().err().unwrap();
        // handle error
    }

    let request = boxed_request.unwrap();

    let boxed_path: Result<String, String> = request.get_path();
    if boxed_path.is_err() {
        // handle error
    }

    assert!(boxed_path.is_ok());
    assert_eq!(boxed_path.unwrap(), "/path");

    let query_option = request.get_query().unwrap();
    assert_eq!(query_option.is_some(), true);

    let query = query_option.unwrap();
    assert_eq!(query.get("key").unwrap(), "5678");
    assert_eq!(query.get("some").unwrap(), "1234");
}
#[test]
fn normalize_request_target_follows_rfc_9112_forms() {
    let n = Request::normalize_request_target;
    assert_eq!(Some("/a?b=1".to_string()), n("/a?b=1"));
    assert_eq!(Some("*".to_string()), n("*"));
    assert_eq!(Some("/x/y?z".to_string()), n("http://example.com:8080/x/y?z"), "absolute-form -> path");
    assert_eq!(Some("/".to_string()), n("HTTPS://example.com"));
    assert_eq!(Some("/?q".to_string()), n("http://example.com?q"));
    for bad in [":x", "example.com:443", "", "?q", "http:///nohost", "ftp://x/"] {
        assert_eq!(None, n(bad), "{bad:?}");
    }
}

#[test]
fn request_with_a_junk_target_is_rejected_not_passed_on() {
    // Used to reach StaticResourceController and panic a worker thread.
    assert!(Request::parse(b"GET :x HTTP/1.1\r\nHost: a\r\n\r\n").is_err());
    let absolute = Request::parse(b"GET http://a.example/p?q=1 HTTP/1.1\r\nHost: a.example\r\n\r\n").unwrap();
    assert_eq!("/p?q=1", absolute.request_uri);
}

#[test]
fn invalid_content_length_is_an_error_not_a_panic() {
    // Found by fuzzing: `.parse().unwrap()` on a client-supplied value.
    for value in ["abc", "-1", "1e3", "99999999999999999999999"] {
        let raw = format!("POST /api HTTP/1.1\r\nHost: a\r\nContent-Length: {value}\r\n\r\n");
        assert!(Request::parse(raw.as_bytes()).is_err(), "{value}");
    }
    let ok = Request::parse(b"POST /api HTTP/1.1\r\nHost: a\r\ncontent-length: 2\r\n\r\nhi").unwrap();
    assert_eq!(b"hi".to_vec(), ok.body, "lower-case header name honoured");
}

#[test]
fn header_values_keep_colons_and_tolerate_missing_spaces() {
    let h = Request::parse_http_request_header_string("Referer: http://example.com:8080/a: b\r\n");
    assert_eq!(("Referer", "http://example.com:8080/a: b"), (h.name.as_ref(), h.value.as_ref()));
    let h = Request::parse_http_request_header_string("X-Compact:value\r\n");
    assert_eq!(("X-Compact", "value"), (h.name.as_ref(), h.value.as_ref()));
    let h = Request::parse_http_request_header_string("X-Padded: \t value \t\r\n");
    assert_eq!("value", h.value);
    let h = Request::parse_http_request_header_string("Host:\r\n");
    assert_eq!(("Host", ""), (h.name.as_ref(), h.value.as_ref()));
}

#[test]
fn method_and_version_checks_ignore_case() {
    assert!(Request::is_supported_method("get") && Request::is_supported_method("PATCH"));
    assert!(!Request::is_supported_method("BREW"));
    assert!(Request::is_supported_method("PURGE"));
    assert_eq!(Request::parse(b"PURGE /a/* HTTP/1.1\r\nHost: x\r\n\r\n").unwrap().method, "PURGE");
    assert!(crate::http::HTTP::is_supported_version("http/1.1"));
    assert!(!crate::http::HTTP::is_supported_version("HTTP/4.0"));
}

/// `Request::parse` (single pass) against the parser it replaced, kept
/// verbatim in `reference_parser.rs`: same fields or the same error, for a
/// corpus of request heads and deterministic mutations of each.
#[test]
fn request_parse_matches_the_reference_parser() {
    fn summary(result: &Result<crate::request::Request, String>) -> String {
        match result {
            Err(e) => format!("Err({e})"),
            Ok(r) => format!("{:?} {:?} {:?} {:?} {:?}", r.method, r.request_uri, r.http_version, r.headers, r.body),
        }
    }
    let corpus: Vec<&[u8]> = vec![
        b"GET /a?b=1 HTTP/1.1\r\nHost: x\r\nAccept: */*\r\n\r\n",
        b"GET / HTTP/1.1\nHost: lf-only\n\nbody",
        b"POST /p HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc",
        b"POST /p HTTP/1.1\r\nContent-Length: x3\r\n\r\nabc",
        b"GET http://example.com/x?y HTTP/1.1\r\nHost: example.com\r\n\r\n",
        b"OPTIONS * HTTP/1.1\r\n\r\n",
        b"GET example.com HTTP/1.1\r\n\r\n",
        b"get /lower HTTP/1.1\r\n\r\n",
        b"GET /a HTTP/9.9\r\n\r\n",
        b"GET /a\r\n\r\n",
        b"GET  /a HTTP/1.1\r\n\r\n",
        b"",
        b"\r\n",
        b"GET /a HTTP/1.1\r\nNo-Colon\r\nA:b\r\nC:  spaced \t\r\nD: x: y\r\n\r\n",
        b"GET /a HTTP/1.1\r\n \t\r\nafter: blank\r\n\r\n",
        b"GET /a HTTP/1.1\r\nX: \xff\r\n\r\n",
        b"GET /\xc3\xa9 HTTP/1.1\r\nX: \xc3\xa9\r\n\r\n\xff\x00",
        b"GET /a HTTP/1.1\r\nX: y",
        b"GET /a HTTP/1.1",
        b"GET /a HTTP/1.1\r\n:\r\n: v\r\n\r\n",
    ];
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    let interesting = [b'\r', b'\n', b' ', b':', b'\t', 0xff, 0xc3, b'0', b'/', b'*'];
    let mut checked = 0;
    for input in &corpus {
        let mut cases = vec![input.to_vec()];
        for _ in 0..200 {
            let mut m = input.to_vec();
            for _ in 0..1 + next() % 3 {
                let r = next();
                match r % 3 {
                    0 if !m.is_empty() => { let i = (r >> 8) as usize % m.len(); m[i] = interesting[(r >> 32) as usize % interesting.len()]; }
                    1 => { let i = (r >> 8) as usize % (m.len() + 1); m.insert(i, interesting[(r >> 32) as usize % interesting.len()]); }
                    _ if !m.is_empty() => { let i = (r >> 8) as usize % m.len(); m.remove(i); }
                    _ => {}
                }
            }
            cases.push(m);
        }
        for case in cases {
            let ours = crate::request::Request::parse(&case);
            let reference = super::reference_parser::parse(&case);
            assert_eq!(summary(&reference), summary(&ours), "input {:?}", String::from_utf8_lossy(&case));
            checked += 1;
        }
    }
    assert!(checked > 4000);
}
