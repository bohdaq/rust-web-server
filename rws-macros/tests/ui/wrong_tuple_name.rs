// Each name bound in a `Path` tuple must be a parameter of the route.
use rws_macros::get;

#[get("/users/{user}/posts/:id")]
fn post(Path((user, slug)): Path<(String, u64)>) -> String {
    format!("{user} {slug}")
}

fn main() {}
