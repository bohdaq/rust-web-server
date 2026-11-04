// A `Path` argument needs route parameters to take.
use rws_macros::get;

#[get("/posts")]
fn list(Path(id): Path<u64>) -> String {
    format!("{id}")
}

fn main() {}
