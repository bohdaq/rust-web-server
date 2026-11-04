// `#[derive(Factory)]` needs the `#[primary_key]` field `#[derive(Model)]` has.
use rws_macros::Factory;

#[derive(Factory)]
struct Note {
    id: i64,
    text: String,
}

fn main() {}
