use std::fs::File;
use std::io::Read;

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