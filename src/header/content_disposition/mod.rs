use crate::symbol::{SYMBOL};

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

        let mut filename = None;
        let mut fieldname = None;

        let boxed_second_element = parts.get(1);
        if boxed_second_element.is_some() {
            let second_element = boxed_second_element.unwrap();
            let boxed_split = second_element.split_once(SYMBOL.equals);
            if boxed_split.is_none() {
                let message = format!("Unable to parse second property in the Content-Disposition header: {}", second_element);
                return Err(message)
            }
            let (key, value) = boxed_split.unwrap();
            let key = key.trim();
            let is_filename_field = key == "filename";
            if is_filename_field {
                filename = Some(value.to_string().replace(SYMBOL.quotation_mark, SYMBOL.empty_string));
            }
            let is_name_field = key == "name";
            if is_name_field  {
                fieldname = Some(value.to_string().replace(SYMBOL.quotation_mark, SYMBOL.empty_string));
            }
        }

        let boxed_third_element = parts.get(2);
        if boxed_third_element.is_some() {
            let second_element = boxed_third_element.unwrap();
            let boxed_split = second_element.split_once(SYMBOL.equals);
            if boxed_split.is_none() {
                let message = format!("Unable to parse second property in the Content-Disposition header: {}", second_element);
                return Err(message)
            }
            let (key, value) = boxed_split.unwrap();
            let key = key.trim();
            let is_filename_field = key == "filename";
            if is_filename_field {
                filename = Some(value.to_string().replace(SYMBOL.quotation_mark, SYMBOL.empty_string));
            }
            let is_name_field = key == "name";
            if is_name_field  {
                fieldname = Some(value.to_string().replace(SYMBOL.quotation_mark, SYMBOL.empty_string));
            }

            if !is_filename_field && !is_name_field {
                let message = format!("Unable to parse property in the Content-Disposition header: {}", raw_content_disposition);
                return Err(message.to_string())
            }
        }

        let content_disposition = ContentDisposition{
            disposition_type: disposition_type.to_string(),
            field_name: fieldname,
            file_name: filename,
        };

        Ok(content_disposition)
    }
}