use crate::header::content_disposition::ContentDisposition;

#[test]
fn inline() {
    let raw_content_disposition = "inline";
    let boxed_content_disposition = ContentDisposition::parse(raw_content_disposition);
}