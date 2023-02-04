#[cfg(test)]
mod tests;

pub struct ContentDisposition {
    pub content_type: String,
    pub field_name: Option<String>,
    pub file_name: Option<String>
}

impl ContentDisposition {
    pub fn parse(raw_content_disposition: &str) -> Result<ContentDisposition, String> {
        let content_disposition = ContentDisposition{
            content_type: "".to_string(),
            field_name: None,
            file_name: None,
        };

        Ok(content_disposition)
    }
}