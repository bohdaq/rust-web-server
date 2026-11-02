//! rws-http tests that read fixtures from the repository root (`static/`,
//! `rws.config.toml`): a crate's tests run in the crate's directory, the
//! facade's in the root, so they live here.

mod body_multipart_form_data {
    use file_ext::FileExt;
    use crate::body::multipart_form_data::{FormMultipartData, Part};
    use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};
    use crate::header::Header;
    use crate::symbol::SYMBOL;

    #[test]
    fn generate() {
        let mut part_list = vec![];

        let content_disposition = ContentDisposition {
            disposition_type: DISPOSITION_TYPE.form_data.to_string(),
            field_name: Some("field1".to_string()),
            file_name: None,
        };

        let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

        let filename = "content.png";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());


        let first_body = boxed_payload.unwrap();
        let part = Part { headers: vec![header.clone()], body: first_body.clone() };

        part_list.push(part);

        let content_disposition = ContentDisposition {
            disposition_type: DISPOSITION_TYPE.form_data.to_string(),
            field_name: Some("field2".to_string()),
            file_name: None,
        };

        let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

        let filename = "audio.m4a";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());


        let second_body = boxed_payload.unwrap();
        let part = Part { headers: vec![header.clone()], body: second_body.clone() };

        part_list.push(part);


        let boundary = "------someboundary------";
        let actual_form = FormMultipartData::generate(part_list, boundary).unwrap();

        let form = FormMultipartData::parse(&actual_form, boundary.to_string()).unwrap();
        assert_eq!(form.len(), 2);

        let first_part = form.get(0).unwrap();
        assert_eq!(first_part.body.len(), first_body.len());
        assert_eq!(first_part.headers.len(), 1);
        assert_eq!(first_part.body, first_body);

        let first_content_disposition = first_part.get_header("Content-Disposition".to_string()).unwrap();
        assert_eq!(first_content_disposition.name, "Content-Disposition");
        assert_eq!(first_content_disposition.value, "form-data; name=\"field1\"");

        let second_part = form.get(1).unwrap();
        assert_eq!(second_part.body.len(), second_body.len());
        assert_eq!(second_part.headers.len(), 1);
        assert_eq!(second_part.body, second_body);

        let second_content_disposition = second_part.get_header("Content-Disposition".to_string()).unwrap();
        assert_eq!(second_content_disposition.name, "Content-Disposition");
        assert_eq!(second_content_disposition.value, "form-data; name=\"field2\"");
    }

    #[test]
    fn parse_multipart_request_body() {
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
            "\n".to_string(),
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
            String::from_utf8(payload.clone()).unwrap(), // payload is not escaped, text file used for test
            new_line.to_string(),
        ].join(SYMBOL.empty_string);

        let raw_payload = [
            raw_payload_key_value,
            raw_payload_another_key_value,
            raw_payload_file,
            boundary.to_string(),
        ].join(SYMBOL.empty_string);

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let part_list = FormMultipartData::parse(raw_payload.as_bytes(), actual_boundary).unwrap();
        let number_of_parts = 3;
        assert_eq!(part_list.len(), number_of_parts);

        let first_part = part_list.get(0).unwrap();
        let disposition : &Header = first_part.get_header("Content-Disposition".to_string()).unwrap();
        let content_disposition = ContentDisposition::parse(&disposition.value).unwrap();
        assert_eq!(content_disposition.field_name.unwrap(), "some");
        assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
        assert_eq!(content_disposition.file_name, None);
        assert_eq!("123".as_bytes(), first_part.body);

        let second_part = part_list.get(1).unwrap();
        let disposition : &Header = second_part.get_header("Content-Disposition".to_string()).unwrap();
        let content_disposition = ContentDisposition::parse(&disposition.value).unwrap();
        assert_eq!(content_disposition.field_name.unwrap(), "key");
        assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
        assert_eq!(content_disposition.file_name, None);
        assert_eq!("45678".as_bytes(), second_part.body);

        let third_part = part_list.get(2).unwrap();
        let disposition : &Header = third_part.get_header("Content-Disposition".to_string()).unwrap();
        let content_disposition = ContentDisposition::parse(&disposition.value).unwrap();
        assert_eq!(content_disposition.field_name.unwrap(), "fileupload");
        assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
        assert_eq!(content_disposition.file_name.unwrap(), "rws.config.toml");
        assert_eq!(payload, third_part.body);

    }

    #[test]
    fn parse_multipart_request_body_image() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "content.png";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let key = "fileupload";
        let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
        let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            payload_boundary.as_bytes(),
            content_disposition.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = [
            &raw_payload_file,
            boundary.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let part_list = FormMultipartData::parse(&raw_payload, actual_boundary).unwrap();
        let number_of_parts = 1;
        assert_eq!(part_list.len(), number_of_parts);

        let first_part = part_list.get(0).unwrap();
        let disposition : &Header = first_part.get_header("Content-Disposition".to_string()).unwrap();
        let content_disposition = ContentDisposition::parse(&disposition.value).unwrap();
        assert_eq!(content_disposition.field_name.unwrap(), "fileupload");
        assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
        assert_eq!(content_disposition.file_name.unwrap(), "content.png");
        assert_eq!(payload.len(), first_part.body.len());

    }

    #[test]
    fn generate_part_image() {
        let content_disposition = ContentDisposition {
            disposition_type: DISPOSITION_TYPE.form_data.to_string(),
            field_name: Some("field1".to_string()),
            file_name: None,
        };

        let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

        let filename = "content.png";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());


        let body = boxed_payload.unwrap();
        let part = Part { headers: vec![header], body: body.clone() };
        let boxed_part = FormMultipartData::generate_part(part);
        assert!(boxed_part.is_ok());

        let expected_part: Vec<u8> = [
            "Content-Disposition: form-data; name=\"field1\"\r\n".as_bytes().to_vec(),
            "\r\n".as_bytes().to_vec(),
            body.clone()
        ].join(SYMBOL.empty_string.as_bytes());

        assert_eq!(expected_part, boxed_part.unwrap())
    }

    #[test]
    fn parse_multipart_request_body_audio() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "audio.m4a";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let key = "fileupload";
        let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
        let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            payload_boundary.as_bytes(),
            content_disposition.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = [
            &raw_payload_file,
            boundary.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let part_list = FormMultipartData::parse(&raw_payload, actual_boundary).unwrap();
        let number_of_parts = 1;
        assert_eq!(part_list.len(), number_of_parts);

        let first_part = part_list.get(0).unwrap();
        let disposition : &Header = first_part.get_header("Content-Disposition".to_string()).unwrap();
        let content_disposition = ContentDisposition::parse(&disposition.value).unwrap();
        assert_eq!(content_disposition.field_name.unwrap(), "fileupload");
        assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
        assert_eq!(content_disposition.file_name.unwrap(), "audio.m4a");
        assert_eq!(payload.len(), first_part.body.len());

    }

    #[test]
    fn parse_multipart_request_body_image_extra_new_line_before_starting_payload_boundary() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "content.png";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let key = "fileupload";
        let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
        let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            new_line.as_bytes(),
            payload_boundary.as_bytes(),
            content_disposition.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = [
            &raw_payload_file,
            boundary.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let actual_error_message = FormMultipartData::parse(&raw_payload, actual_boundary).err().unwrap();
        let expected_error_message = "Body in multipart/form-data request needs to start with a boundary, actual string: ''";
        assert_eq!(actual_error_message, expected_error_message);

    }

    #[test]
    fn parse_multipart_request_body_image_no_start_boundary() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "content.png";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let key = "fileupload";
        let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            content_disposition.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = [
            &raw_payload_file,
            boundary.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let actual_error_message = FormMultipartData::parse(&raw_payload, actual_boundary).err().unwrap();
        let expected_error_message = "Body in multipart/form-data request needs to start with a boundary, actual string: 'Content-Disposition: form-data; name=\"fileupload\"; filename=\"content.png\"'";
        assert_eq!(actual_error_message, expected_error_message);

    }

    #[test]
    fn parse_multipart_request_body_video_no_content_disposition() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "video.mov";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            payload_boundary.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = [
            &raw_payload_file,
            boundary.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let actual_error = FormMultipartData::parse(&raw_payload, actual_boundary).err().unwrap();
        let expected_error = "One of the body parts does not have any header specified. At least Content-Disposition is required";
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn parse_multipart_request_body_image_no_end_boundary() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "content.png";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let key = "fileupload";
        let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
        let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            payload_boundary.as_bytes(),
            content_disposition.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = raw_payload_file;

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let actual_error_message = FormMultipartData::parse(&raw_payload, actual_boundary).err().unwrap();
        let expected_error_message = "No end boundary present in the multipart/form-data request body";
        assert_eq!(actual_error_message, expected_error_message);

    }

    #[test]
    fn parse_multipart_request_body_video() {
        let boundary = "------hdfkjshdfkljashdgkh";


        let new_line = SYMBOL.new_line_carriage_return.to_string();

        let filename = "video.mov";
        let path = FileExt::build_path(&["static", filename]);
        let boxed_payload = FileExt::read_file(&path);
        assert!(boxed_payload.is_ok());

        let payload = boxed_payload.unwrap();
        let key = "fileupload";
        let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
        let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
        let raw_payload_file = [
            payload_boundary.as_bytes(),
            content_disposition.as_bytes(),
            new_line.as_bytes(),
            &payload.clone(),
            new_line.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let raw_payload = [
            &raw_payload_file,
            boundary.as_bytes(),
        ].join(SYMBOL.empty_string.as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
        assert_eq!(actual_boundary, boundary);

        let part_list = FormMultipartData::parse(&raw_payload, actual_boundary).unwrap();
        let number_of_parts = 1;
        assert_eq!(part_list.len(), number_of_parts);

        let first_part = part_list.get(0).unwrap();
        let disposition : &Header = first_part.get_header("Content-Disposition".to_string()).unwrap();
        let content_disposition = ContentDisposition::parse(&disposition.value).unwrap();
        assert_eq!(content_disposition.field_name.unwrap(), "fileupload");
        assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
        assert_eq!(content_disposition.file_name.unwrap(), "video.mov");
        assert_eq!(payload.len(), first_part.body.len());

    }
}

mod body_example {
    use crate::header::Header;
    use crate::http::VERSION;
    use crate::range::{ContentRange, Range};
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

    #[test]
    fn multipart_body_in_response() {
        let resource_uri = "/static/content.png";

        // building body (content_range_list)
        let range = Header {
            name: Header::_RANGE.into(),
            value: "bytes=200-1000, 1200-1400".into()
        };

        let content_range_list : Vec<ContentRange> = Range::get_content_range_list(resource_uri, &range).unwrap();

        // building response
        let host = Header {
            name: Header::_HOST.into(),
            value: "localhost".into()
        };

        let content_type = Header {
            name: Header::_CONTENT_TYPE.into(),
            value: Range::MULTIPART_BYTERANGES_CONTENT_TYPE.into()
        };

        let response = Response {
            http_version: VERSION.http_1_1.to_string(),
            status_code: *STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code,
            reason_phrase: STATUS_CODE_REASON_PHRASE.n206_partial_content.reason_phrase.to_string(),
            headers: vec![host, content_type],
            content_range_list, // same as `content_range_list: content_range_list`
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        };


        // replace with your logic
        let response_body : &ContentRange = response.content_range_list.get(0).unwrap();
        assert_eq!(801, response_body.body.len());

        let response_body : &ContentRange = response.content_range_list.get(1).unwrap();
        assert_eq!(201, response_body.body.len());

    }
}

mod request {
    use file_ext::FileExt;
    use crate::request::{Request};
    use crate::symbol::SYMBOL;

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
}

mod response {
    use std::borrow::Borrow;
    use std::env;
    use std::fs::File;
    use std::io::Read;
    use crate::http::VERSION;
    use crate::mime_type::MimeType;
    use crate::range::{ContentRange, Range};
    use crate::request::{METHOD, Request};
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::symbol::SYMBOL;

    #[test]
    fn it_generates_successful_response_with_additional_headers_and_non_utf8_file() {
        let response_http_version = VERSION.http_1_1.to_string();
        let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;
        let filepath = "/static/content.png";

        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();

        let mut response_filepath = [working_directory, filepath].join(SYMBOL.empty_string);
        let mut contents = Vec::new();
        let mut file = File::open(response_filepath).unwrap();
        file.read_to_end(&mut contents).expect("Unable to read");

        let response_content_length_header_name = "Content-Length";
        let response_content_length_header_value = contents.len().to_string();

        let headers = vec![];

        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range {
                start: 0,
                end: contents.len() as u64
            },
            size: contents.len().to_string(),
            body: contents.to_vec(),
            content_type: MimeType::IMAGE_PNG.to_string()
        };

        let response = Response {
            http_version: response_http_version.to_string(),
            status_code: *response_status_code,
            reason_phrase: response_reason_phrase.to_string(),
            headers,
            content_range_list: vec![content_range],
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        };

        let request = Request {
            method: METHOD.get.to_string(),
            request_uri: "/some-route".to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: Default::default(),
        };

        let raw_response = Response::generate_response(response, request);
        let response = Response::_parse_response(raw_response.borrow());


        let content_length_header = response._get_header(response_content_length_header_name.to_string()).unwrap();
        assert_eq!(response_content_length_header_value, content_length_header.value);


        assert_eq!(response_http_version, response.http_version);
        assert_eq!(*response_status_code, response.status_code);
        assert_eq!(response_reason_phrase, response.reason_phrase);

        contents = Vec::new();
        response_filepath = [working_directory, filepath].join(SYMBOL.empty_string);
        file = File::open(response_filepath).unwrap();
        file.read_to_end(&mut contents).expect("Unable to read");
        assert_eq!(contents, response.content_range_list.get(0).unwrap().body);
    }
}
