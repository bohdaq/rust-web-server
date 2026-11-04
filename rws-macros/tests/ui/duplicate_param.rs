// A route can't name a parameter twice.
use rws_macros::get;

#[get("/a/{id}/b/:id")]
fn twice(Path(id): Path<u64>) -> String {
    format!("{id}")
}

fn main() {}
