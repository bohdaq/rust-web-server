use std::collections::HashMap;
use file_ext::FileExt;
use crate::body::form_urlencoded::FormUrlEncoded;
use crate::header::Header;
use crate::http::{VERSION};
use crate::request::{METHOD, Request};

#[test]
fn parse_array_of_bytes_as_request() {

    // retrieve request byte array, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "request", "example", "request.txt"]);
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


    // here goes asserts, this part you can replace with your logic
    let uri = "/form-upload";
    let method = "POST";
    let http_version = "HTTP/1.1";
    let content_type = "application/x-www-form-urlencoded";
    let host = "localhost";
    let body = "some=1234&key=5678";

    assert_eq!(uri, request.request_uri);
    assert_eq!(method, request.method);
    assert_eq!(http_version, request.http_version);

    // how to retrieve header from request
    let content_type_header = request.get_header("Content-Type".to_string()).unwrap();
    assert_eq!(content_type_header.value, content_type);

    let host_header = request.get_header("Host".to_string()).unwrap();
    assert_eq!(host_header.value, host);

    // body is u8 byte array
    assert_eq!(body.as_bytes(), request.body);


    // in this example request body contained url encoded form, here is the sample how to parse it
    let boxed_parse = FormUrlEncoded::parse(request.body);
    let form = boxed_parse.unwrap();

    // asserts for form
    assert_eq!(form.get("key").unwrap(), "5678");
    assert_eq!(form.get("some").unwrap(), "1234");

}


#[test]
fn generate() {
    let mut params_map: HashMap<String, String> = HashMap::new();
    params_map.insert("key1".to_string(), "test1".to_string());
    params_map.insert("key2".to_string(), "test2".to_string());

    let form_url_encoded : String = FormUrlEncoded::generate(params_map);

    let host = Header { name: Header::_HOST.to_string(), value: "localhost".to_string() };

    let endpoint = "/endpoint";

    let request = Request {
        method: METHOD.post.to_string(),
        request_uri: endpoint.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![host],
        body: form_url_encoded.as_bytes().to_vec(),
    };

    let raw_request : Vec<u8> = request.generate();

    // asserts, replace with your logic
    let path = FileExt::build_path(&["src", "request", "example", "expected_request.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let expected_request_file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    assert_eq!(expected_request_file_as_bytes, raw_request);
}