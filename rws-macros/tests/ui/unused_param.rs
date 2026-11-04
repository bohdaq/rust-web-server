// Every route parameter must be taken by a `Path` (or `&PathParams`) argument.
use rws_macros::post;

#[post("/posts/{id}/comments")]
fn comment(State(db): State<Db>) -> String {
    String::new()
}

fn main() {}
