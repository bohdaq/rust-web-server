#[test]
fn parse() {
    let args_vec_as_str : Vec<&str> = "-i=127.0.0.1 -p=7777 -t=100 -a=false -o=http://localhost:7887,http://localhost:8668 -m=GET,POST,PUT,DELETE -h=content-type,x-custom-header -c=true -e=content-type,x-custom-header -g=5555"
        .split_whitespace()
        .collect::<Vec<&str>>();

    let args_vec_as_string : Vec<String> = args_vec_as_str.iter().map(|str| str.to_string()).collect::<Vec<String>>();

    let debug = format!("{:?}", args_vec_as_string);

    assert_eq!("1", debug);
}