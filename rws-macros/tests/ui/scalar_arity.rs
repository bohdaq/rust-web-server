// A scalar `Path` takes exactly one route parameter.
use rws_macros::get;

#[get("/posts/{id}/{slug}")]
fn show_post(Path(id): Path<u64>) -> String {
    format!("{id}")
}

fn main() {}
