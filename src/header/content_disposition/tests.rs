use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};


#[test]
fn not_proper_value() {
    let raw_content_disposition = "sometext";
    let boxed_parse = ContentDisposition::parse(raw_content_disposition);
    let content_disposition_is_error = boxed_parse.is_err();
    assert!(content_disposition_is_error);
    assert_eq!("Unable to parse Content-Disposition header: sometext", boxed_parse.err().unwrap())
}

#[test]
fn inline() {
    let raw_content_disposition = "inline";
    let content_disposition = ContentDisposition::parse(raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, raw_content_disposition);
}

#[test]
fn attachment() {
    let raw_content_disposition = "attachment";
    let content_disposition = ContentDisposition::parse(raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, raw_content_disposition);
}

#[test]
fn attachment_filename() {
    let attachment = "attachment";
    let filename = "somefile";
    let raw_content_disposition = format!("{}; filename=\"{}\"", attachment, filename);
    let content_disposition = ContentDisposition::parse(&raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.attachment);
    assert_eq!(content_disposition.file_name.unwrap(), filename);
    assert_eq!(content_disposition.field_name, None);
}

#[test]
fn form_data_field() {
    let attachment = "form-data";
    let name = "somefield";
    let raw_content_disposition = format!("{}; name=\"{}\"", attachment, name);
    let content_disposition = ContentDisposition::parse(&raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
    assert_eq!(content_disposition.file_name, None);
    assert_eq!(content_disposition.field_name.unwrap(), name);
}

#[test]
fn form_data_field_filename() {
    let attachment = "form-data";
    let name = "somefield";
    let filename = "somefilename";
    let raw_content_disposition = format!("{}; name=\"{}\"; filename=\"{}\"", attachment, name, filename);
    let content_disposition = ContentDisposition::parse(&raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
    assert_eq!(content_disposition.file_name.unwrap(), filename);
    assert_eq!(content_disposition.field_name.unwrap(), name);
}

#[test]
fn not_valid_form_data_field_filename() {
    let attachment = "form-data";
    let name = "somefield";
    let filename = "somefilename";
    let raw_content_disposition = format!("{}; naame=\"{}\"; ffilename=\"{}\"", attachment, name, filename);
    let content_disposition_error = ContentDisposition::parse(&raw_content_disposition).err().unwrap();
    assert_eq!("Unable to parse property in the Content-Disposition header: form-data; naame=\"somefield\"; ffilename=\"somefilename\"", content_disposition_error);
}

#[test]
fn not_valid_form_data_field_filename_v2() {
    let attachment = "form-data";
    let filename = "somefilename";
    let raw_content_disposition = format!("{}; naame; ffilename=\"{}\"", attachment, filename);
    let content_disposition_error = ContentDisposition::parse(&raw_content_disposition).err().unwrap();
    assert_eq!("Unable to parse second property in the Content-Disposition header:  naame", content_disposition_error);
}