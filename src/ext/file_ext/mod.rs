use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use crate::range::Range;

pub struct FileExt;

impl FileExt {
    pub fn read_file_partially(filepath: &str, range: &Range) -> Result<Vec<u8>, String> {
        let mut file_content = Vec::new();

        let buff_length = (range.end - range.start) + 1;
        let file = File::open(filepath).unwrap();
        let mut reader = BufReader::new(file);

        let boxed_seek = reader.seek(SeekFrom::Start(range.start));
        if boxed_seek.is_ok() {
            let boxed_read = reader.take(buff_length).read_to_end(&mut file_content);
            if boxed_read.is_err() {
                return Err(boxed_read.err().unwrap().to_string())
            }
        } else {
            return Err(boxed_seek.err().unwrap().to_string())
        }

        Ok(file_content)
    }

    pub fn read_file(filepath: &str) -> Result<Vec<u8>, String> {

        let mut file_content = Vec::new();
        let boxed_open = File::open(filepath);
        if boxed_open.is_err() {
            let error_msg = boxed_open.err().unwrap();
            let error = format!("<p>Unable to open file: {}</p> <p>error: {}</p>", filepath, error_msg);
            return Err(error)
        } else {
            let mut file = boxed_open.unwrap();
            let boxed_read= file.read_to_end(&mut file_content);
            if boxed_read.is_err() {
                let error_msg = boxed_read.err().unwrap();
                let error = format!("<p>Unable to read file: {}</p> <p>error: {}</p>", filepath, error_msg);
                return Err(error)
            }
        }
        Ok(file_content)
    }
}

