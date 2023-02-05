use file_ext::FileExt;
use crate::body::multipart_form_data::FormMultipartData;
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
        String::from_utf8(payload).unwrap(), // payload is not escaped, text file used for test
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

    FileExt::create_file("out.log").unwrap();
    FileExt::write_file("out.log", raw_payload.len().to_string().as_bytes()).unwrap();
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
    // TODO:

}