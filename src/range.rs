use std::io::prelude::*;
use std::net::TcpStream;
use std::{env, fs, io};
use std::borrow::Borrow;
use std::char::MAX;
use std::fs::{File, metadata};
use std::io::BufReader;

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::{CONSTANTS, Server};
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::header::Header;


pub struct Range {}


impl Range {

    pub(crate) fn get_exact_start_and_end_of_file(request_uri: &str, range: &Header) -> (usize, usize, usize) {
        let raw_range_value = &range.header_value;
        println!("raw_range_value: {}", raw_range_value);
        let split_raw_range_value: Vec<&str> = raw_range_value.split(CONSTANTS.EQUALS).collect();
        let raw_bytes = split_raw_range_value.get(1).unwrap();

        println!("split_raw_range_value: {}", raw_bytes);


        let mut start: usize = 0;
        let mut end: usize = 0;
        let mut length: usize = 0;

        let mut contents = Vec::new();
        let static_filepath = Server::get_static_filepath(request_uri);
        let boxed_file = File::open(&static_filepath);
        if boxed_file.is_ok()  {
            let md = metadata(&static_filepath).unwrap();
            if md.is_file() {
                let mut file = boxed_file.unwrap();
                length = md.len() as usize;
                file.read_to_end(&mut contents).expect("Unable to read");
            }
        }



        return (start, end, length)
    }
}


