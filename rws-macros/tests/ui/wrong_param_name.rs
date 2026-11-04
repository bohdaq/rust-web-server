// The binding inside `Path(…)` names the route parameter it takes.
use rws_macros::get;

#[get("/posts/{id}")]
fn show_post(Path(post_id): Path<u64>) -> String {
    format!("{post_id}")
}

fn main() {}
