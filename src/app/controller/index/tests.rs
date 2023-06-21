use std::fs;
use file_ext::FileExt;
use crate::app::controller::index::IndexController;
use crate::controller::Controller;
use crate::core::New;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::response::{Response};
use crate::server::{Address, ConnectionInfo};

#[test]
fn file_retrieval() {
    // user provided html file
    let pwd = FileExt::working_directory().unwrap();

    let path = "/";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    };

    let is_matching = IndexController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = IndexController::process(&request, response, &connection_info);


    let path_array = vec![pwd.as_str(), "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text.to_vec());




    // default index.html
    let progress = |start, end, total| println!("progress {} of {}", end, total);
    copy_file(vec![pwd.as_str(), "index.html"], vec![pwd.as_str(), "index_copy.html"], progress).unwrap();

    FileExt::delete_file("index.html").unwrap();

    let path = "/";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    };

    let is_matching = IndexController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = IndexController::process(&request, response, &connection_info);


    let path_array = vec![pwd.as_str(), "src", "app", "controller", "index", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text.to_vec());

    copy_file(vec![pwd.as_str(), "index_copy.html"], vec![pwd.as_str(), "index.html"], progress).unwrap();
    FileExt::delete_file("index_copy.html").unwrap();
}

fn file_length(path: Vec<&str>) -> Result<u64, String> {
    let filepath = FileExt::build_path(path.as_slice());
    let boxed_length = fs::metadata(filepath);
    if boxed_length.is_err() {
        let message = boxed_length.err().unwrap().to_string();
        return Err(message)
    }
    let length = boxed_length.unwrap().len();
    Ok(length)
}

fn copy_file<F: Fn(u64, u64, u64)>(from: Vec<&str>, to: Vec<&str>, f: F)-> Result<(), String> {
    let boxed_length = file_length(from.clone());
    if boxed_length.is_err() {
        let message = boxed_length.err().unwrap();
        return Err(message);
    }

    let file_length = boxed_length.unwrap();
    let _100kb = 102400;
    let step = _100kb;
    let mut start = 0;
    let mut end = step;
    if step >= file_length {
        end = file_length - 1;
    }

    let mut continue_copying = true;
    while continue_copying {
        f(start, end, file_length);

        let boxed_copy = copy_part_of_file(
            from.clone(),
            to.clone(),
            start,
            end
        );

        if boxed_copy.is_err() {
            let message = boxed_copy.err().unwrap();
            return Err(message);
        }

        boxed_copy.unwrap();

        if end == file_length - 1 {
            continue_copying = false;
        } else {
            start = end + 1;
            end = end + step;
            if start + step >= file_length {
                end = file_length - 1;
            }
        }

    }

    Ok(())
}

fn _copy_file(from: Vec<&str>, to: Vec<&str>) -> Result<(), String> {
    let from_path = FileExt::build_path(&from);
    let file_exists = FileExt::does_file_exist(from_path.as_str());
    if !file_exists {
        let message = format!("file at given path {} does not exist", from_path.as_str());
        return Err(message);
    }

    let to_path = FileExt::build_path(&to);
    let file_exists = FileExt::does_file_exist(to_path.as_str());
    if file_exists {
        let message = format!("file at given path {} already exists", to_path.as_str());
        return Err(message);
    }

    let boxed_content_to_copy = FileExt::read_file(from_path.as_str());
    if boxed_content_to_copy.is_err() {
        let message = boxed_content_to_copy.err().unwrap();
        return Err(message);
    }
    let content_to_copy = boxed_content_to_copy.unwrap();


    let to_path = FileExt::build_path(&to);
    if FileExt::does_file_exist(to_path.as_str()) {
        let boxed_delete = FileExt::delete_file(to_path.as_str());
        if boxed_delete.is_err() {
            let message = boxed_delete.err().unwrap();
            return Err(message);
        }
    }
    let boxed_create = FileExt::create_file(to_path.as_str());
    if boxed_create.is_err() {
        let message = boxed_create.err().unwrap();
        return Err(message);
    }

    let boxed_write =
        FileExt::write_file(to_path.as_str(), content_to_copy.as_slice());
    if boxed_write.is_err() {
        let message = boxed_write.err().unwrap();
        return Err(message);
    }
    Ok(boxed_write.unwrap())
}


fn copy_part_of_file(from: Vec<&str>, to: Vec<&str>, start: u64, end: u64) -> Result<(), String> {
    let from_path = FileExt::build_path(&from);
    let file_exists = FileExt::does_file_exist(from_path.as_str());
    if !file_exists {
        let message = format!("file at given path {} does not exist", from_path.as_str());
        return Err(message);
    }


    let boxed_content_to_copy = FileExt::read_file_partially(from_path.as_str(), start, end);
    if boxed_content_to_copy.is_err() {
        let message = boxed_content_to_copy.err().unwrap();
        return Err(message);
    }
    let content_to_copy = boxed_content_to_copy.unwrap();


    let to_path = FileExt::build_path(&to);
    if !FileExt::does_file_exist(to_path.as_str()) {
        let boxed_create = FileExt::create_file(to_path.as_str());
        if boxed_create.is_err() {
            let message = boxed_create.err().unwrap();
            return Err(message);
        }
    }


    let boxed_write =
        FileExt::write_file(to_path.as_str(), content_to_copy.as_slice());
    if boxed_write.is_err() {
        let message = boxed_write.err().unwrap();
        return Err(message);
    }
    Ok(boxed_write.unwrap())
}