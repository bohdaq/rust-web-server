use crate::header::content_disposition::ContentDisposition;

#[test]
fn inline() {
    let raw_content_disposition = "inline";
    let content_disposition = ContentDisposition::parse(raw_content_disposition).unwrap();
    assert_eq!(content_disposition.disposition_type, raw_content_disposition);
}