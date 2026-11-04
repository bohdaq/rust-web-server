// `#[timestamps]` needs `created_at` and `updated_at` fields.
use rws_macros::Model;

#[derive(Model)]
#[timestamps]
struct Note {
    #[primary_key(auto_increment)]
    id: i64,
    created_at: String,
}

fn main() {}
