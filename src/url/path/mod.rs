use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub struct UrlPath;

impl UrlPath {
    pub fn is_matching(_path: &str, _pattern: &str) -> Result<bool, String> {
        //TODO

        let mut map = HashMap::new();
        map.insert("qwert", "123");
        map.insert("qwert", "123");
        Ok(true)
    }

    pub fn extract(_path: &str, _pattern: &str) -> Result<HashMap<String, String>, String> {
        //TODO

        let mut map = HashMap::new();
        Ok(map)
    }
}