use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub struct UrlPath;

impl UrlPath {
    pub fn is_matching(_path: &str, _pattern: &str) -> Result<bool, String> {
        //TODO

        // extract static parts and name of keys
        for _char in _path.chars() {
            if _char.is_whitespace() || _char.is_ascii_control() {
                return Err("path contains control character or whitespace".to_string())
            }
        }

        Ok(true)
    }

    pub fn extract(_path: &str, _pattern: &str) -> Result<HashMap<String, String>, String> {
        //TODO

        let mut map = HashMap::new();
        Ok(map)
    }

    pub fn build(_params: HashMap<String, String>, _pattern: &str) -> Result<String, String> {
        //TODO

        let mut _map : HashMap<String, String> = HashMap::new();
        Ok("generated path here".to_string())
    }
}