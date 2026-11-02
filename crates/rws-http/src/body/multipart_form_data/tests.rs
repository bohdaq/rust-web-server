use crate::body::multipart_form_data::{FormMultipartData, Part};
use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};
use crate::header::Header;
use crate::symbol::SYMBOL;

#[test]
fn parse_multipart_request_body_malformed_content_disposition_header() {
    let content_disposition = format!("form-data; typoname=\"{}\"", "key");
    let actual_error_content_disposition = ContentDisposition::parse(&content_disposition).err().unwrap();
    assert_eq!(actual_error_content_disposition, "Field name is not set for Content-Disposition: form-data; typoname=\"key\"");
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
fn generate_part_no_headers() {
    let part = Part { headers: vec![], body: Default::default() };
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

