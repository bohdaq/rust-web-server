// A typed handler returns something that implements `IntoResponse`.
use rws_macros::get;

#[get("/ping")]
fn ping(State(db): State<Db>) {}

fn main() {}
