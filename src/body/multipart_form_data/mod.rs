#[cfg(test)]
mod tests;

use crate::header::Header;

pub struct FormMultipartData;

pub struct Part {
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl FormMultipartData {
    pub fn parse(data: Vec<u8>) -> Result<Vec<Part>, String> {
        let parts = vec![];
        Ok(parts)
    }
}