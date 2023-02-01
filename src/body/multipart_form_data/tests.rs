use file_ext::FileExt;
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

   // TODO:
}