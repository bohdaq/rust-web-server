use file_ext::FileExt;
use crate::body::multipart_form_data::{FormMultipartData, Part};
use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};
use crate::header::Header;
use crate::symbol::SYMBOL;

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


#[test]
fn parse_multipart_request_body_malformed_content_disposition_header() {
    let content_disposition = format!("form-data; typoname=\"{}\"", "key");
    let actual_error_content_disposition = ContentDisposition::parse(&content_disposition).err().unwrap();
    assert_eq!(actual_error_content_disposition, "Field name is not set for Content-Disposition: form-data; typoname=\"key\"");
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
fn parse_multipart_request_body_video_zero_length_payload() {
    let boundary = "------hdfkjshdfkljashdgkh";


    let new_line = SYMBOL.new_line_carriage_return.to_string();

    let filename = "video.mov";

    let payload : Vec<u8> = vec![];
    let key = "fileupload";
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
    let raw_payload_file = [
        payload_boundary.as_bytes(),
        content_disposition.as_bytes(),
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

#[test]
fn parse_multipart_request_body_video_no_header_body_delimiter() {
    let boundary = "------hdfkjshdfkljashdgkh";



    let filename = "video.mov";

    let key = "fileupload";
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}", key, filename, SYMBOL.new_line_carriage_return);
    let raw_payload_file = [
        payload_boundary.as_bytes(),
        content_disposition.as_bytes(),
    ].join(SYMBOL.empty_string.as_bytes());

    let raw_payload = [
        &raw_payload_file,
        boundary.as_bytes(),
    ].join(SYMBOL.empty_string.as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let actual_boundary = FormMultipartData::extract_boundary(&content_type).unwrap();
    assert_eq!(actual_boundary, boundary);

    let actual_error_message = FormMultipartData::parse(&raw_payload, actual_boundary).err().unwrap();
    assert_eq!(actual_error_message, "There is at least one missing body part in the multipart/form-data request");

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
fn generate_part_no_headers() {
    let part = Part { headers: vec![], body: vec![] };
    let boxed_part = FormMultipartData::generate_part(part);
    assert!(boxed_part.is_err());
    assert_eq!("One of the body parts does not have any header specified. At least Content-Disposition is required", boxed_part.err().unwrap())
}

#[test]
fn generate_part() {
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some("field1".to_string()),
        file_name: None,
    };

    let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

    let body = "some-data".as_bytes().to_vec();
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