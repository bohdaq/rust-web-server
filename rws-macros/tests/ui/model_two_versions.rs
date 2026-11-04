// One `#[version]` field at most.
use rws_macros::Model;

#[derive(Model)]
struct Note {
    #[primary_key(auto_increment)]
    id: i64,
    #[version]
    a: i64,
    #[version]
    b: i64,
}

fn main() {}
