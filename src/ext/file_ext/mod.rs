use std::env;
use std::fs::{File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use chrono::{DateTime, Utc};
use crate::ext::date_time_ext::DateTimeExt;
use crate::range::Range;
use crate::symbol::SYMBOL;

pub struct FileExt;

impl FileExt {
    pub fn read_file_partially(filepath: &str, range: &Range) -> Result<Vec<u8>, String> {
        let mut file_content = Vec::new();

        let buff_length = (range.end - range.start) + 1;
        let boxed_open = File::open(filepath);
        if boxed_open.is_err() {
            let error_msg = boxed_open.err().unwrap();
            let error = format!("<p>Unable to open file: {}</p> <p>error: {}</p>", filepath, error_msg);
            return Err(error)
        }

        let file = File::open(filepath).unwrap();
        let mut reader = BufReader::new(file);

        let boxed_seek = reader.seek(SeekFrom::Start(range.start));
        if boxed_seek.is_ok() {
            let boxed_read = reader.take(buff_length).read_to_end(&mut file_content);
            if boxed_read.is_err() {
                let error_msg = boxed_read.err().unwrap().to_string();
                let error = format!("<p>Unable to read file: {}</p> <p>error: {}</p>", filepath, error_msg);
                return Err(error)
            }
        } else {
            let error_msg = boxed_seek.err().unwrap().to_string();
            let error = format!("<p>Unable to seek file: {}</p> <p>error: {}</p>", filepath, error_msg);
            return Err(error)
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

    pub fn file_modified_utc(filepath: &str) -> Result<DateTime<Utc>, String> {

        let boxed_open = File::open(filepath);
        if boxed_open.is_err() {
            let error_msg = boxed_open.err().unwrap();
            let error = format!("<p>Unable to open file: {}</p> <p>error: {}</p>", filepath, error_msg);
            return Err(error)
        }

        let file : File = boxed_open.unwrap();
        let boxed_metadata = file.metadata();
        if boxed_metadata.is_err() {
            let error_msg = boxed_metadata.err().unwrap();
            let error = format!("<p>Unable to open file: {}</p> <p>error: {}</p>", filepath, error_msg);
            return Err(error)
        }
        let metadata = boxed_metadata.unwrap();
        let boxed_last_modified_time = metadata.modified();
        if boxed_last_modified_time.is_err() {
            let error_msg = boxed_last_modified_time.err().unwrap();
            let error = format!("<p>Unable to open file: {}</p> <p>error: {}</p>", filepath, error_msg);
            return Err(error)
        }
        let modified_time = boxed_last_modified_time.unwrap();
        let current_utc: DateTime<Utc> = DateTimeExt::_system_time_to_utc(modified_time);
        Ok(current_utc)
     }

    pub fn get_static_filepath(request_uri: &str) -> String {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        [working_directory, request_uri].join(SYMBOL.empty_string)
    }
}

