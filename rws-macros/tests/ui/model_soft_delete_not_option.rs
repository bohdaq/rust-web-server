// `#[soft_delete]`'s `deleted_at` must be an Option: NULL means not deleted.
use rws_macros::Model;

#[derive(Model)]
#[soft_delete]
struct Note {
    #[primary_key(auto_increment)]
    id: i64,
    deleted_at: String,
}

fn main() {}
