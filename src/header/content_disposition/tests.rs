use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};

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
fn form_data() {
    let attachment = "form-data";
    let name = "somefield";
    let raw_content_disposition = format!("{}; name=\"{}\"", attachment, name);
    let content_disposition = ContentDisposition::parse(&raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, DISPOSITION_TYPE.form_data);
    assert_eq!(content_disposition.file_name, None);
    assert_eq!(content_disposition.field_name.unwrap(), name);
}