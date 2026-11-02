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

#[test]
fn as_string_form_data_no_field() {
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: None,
        file_name: None,
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_err());

    let actual_message = boxed_content_dispostion.err().unwrap();
    assert_eq!("Content-Dispositon header with a type of multipart/form-data is required to have 'name' property", actual_message);
}

#[test]
fn as_string_form_data_field_specified() {
    let field = "somefield".to_string();

    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some(field),
        file_name: None,
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_ok());

    let actual_message = boxed_content_dispostion.unwrap();
    assert_eq!("Content-Disposition: form-data; name=\"somefield\"", actual_message);
}

#[test]
fn as_string_form_data_field_filename() {
    let field = "somefield".to_string();
    let file = "somefile".to_string();

    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some(field),
        file_name: Some(file),
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_ok());

    let actual_message = boxed_content_dispostion.unwrap();
    assert_eq!("Content-Disposition: form-data; name=\"somefield\"; filename=\"somefile\"", actual_message);
}

#[test]
fn as_string_inline_no_field() {
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.inline.to_string(),
        field_name: None,
        file_name: None,
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_ok());

    let actual_message = boxed_content_dispostion.unwrap();
    assert_eq!("Content-Disposition: inline", actual_message);
}

#[test]
fn as_string_attachment_no_field() {
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.attachment.to_string(),
        field_name: None,
        file_name: None,
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_ok());

    let actual_message = boxed_content_dispostion.unwrap();
    assert_eq!("Content-Disposition: attachment", actual_message);
}

#[test]
fn as_string_attachment() {
    let filename = "filename".to_string();
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.attachment.to_string(),
        field_name: None,
        file_name: Some(filename),
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_ok());

    let actual_message = boxed_content_dispostion.unwrap();
    assert_eq!("Content-Disposition: attachment; filename=\"filename\"", actual_message);
}

#[test]
fn as_string_attachment_extra_name() {
    let filename = "filename".to_string();
    let fieldname = "field".to_string();
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.attachment.to_string(),
        field_name: Some(fieldname),
        file_name: Some(filename),
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_err());

    let actual_message = boxed_content_dispostion.err().unwrap();
    assert_eq!("For Content-Disposition of type attachment 'name' property is redundant", actual_message);
}

#[test]
fn as_string_inline_extra_name() {
    let fieldname = "field".to_string();
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.inline.to_string(),
        field_name: Some(fieldname),
        file_name: None,
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_err());

    let actual_message = boxed_content_dispostion.err().unwrap();
    assert_eq!("For Content-Disposition of type inline 'name' property is redundant", actual_message);
}

#[test]
fn as_string_inline_extra_filename() {
    let filename = "filename".to_string();
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.inline.to_string(),
        field_name: None,
        file_name: Some(filename),
    };

    let boxed_content_dispostion = content_disposition.as_string();
    assert!(boxed_content_dispostion.is_err());

    let actual_message = boxed_content_dispostion.err().unwrap();
    assert_eq!("For Content-Disposition of type inline 'filename' property is redundant", actual_message);
}