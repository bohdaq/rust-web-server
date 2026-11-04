// A `Path` tuple takes one element per route parameter.
use rws_macros::get;

#[get("/posts/{id}")]
fn show_post(Path((id, slug)): Path<(u64, String)>) -> String {
    format!("{id} {slug}")
}

fn main() {}
