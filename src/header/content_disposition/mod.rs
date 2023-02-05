use crate::symbol::{Symbol, SYMBOL};

#[cfg(test)]
mod tests;

pub struct ContentDisposition {
    pub disposition_type: String,
    pub field_name: Option<String>,
    pub file_name: Option<String>
}

pub struct DispositionType {
    pub inline: &'static str,
    pub attachment: &'static str,
    pub form_data: &'static str
}

pub const DISPOSITION_TYPE: DispositionType = DispositionType {
    inline: "inline",
    attachment: "attachment",
    form_data: "form-data",
};


impl ContentDisposition {
    pub fn parse(raw_content_disposition: &str) -> Result<ContentDisposition, String> {
        let mut parts: Vec<&str> = raw_content_disposition.split(SYMBOL.semicolon).collect();
        if parts.len() == 0 {
            parts.push(raw_content_disposition);
        }

        let disposition_type = parts.get(0).unwrap();
        if disposition_type.to_string() != DISPOSITION_TYPE.inline.to_string()
            && disposition_type.to_string() != DISPOSITION_TYPE.attachment.to_string()
            && disposition_type.to_string() != DISPOSITION_TYPE.form_data.to_string() {
            let message = format!("Unable to parse Content-Disposition header: {}", raw_content_disposition);
            return Err(message)
        }

        let content_disposition = ContentDisposition{
            disposition_type: disposition_type.to_string(),
            field_name: None,
            file_name: None,
        };

        Ok(content_disposition)
    }
}