//! Tests of rws-server's `di` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

use std::sync::Arc;
use crate::di::Container;
use crate::application::Application;
use crate::request::Request;
use crate::server::ConnectionInfo;

trait Greeter: Send + Sync {
    fn greet(&self) -> &str;
}

struct Hello;

impl Greeter for Hello {
    fn greet(&self) -> &str { "hello" }
}

struct Hi;

impl Greeter for Hi {
    fn greet(&self) -> &str { "hi" }
}

#[test]
fn container_usable_as_with_state() {
    use crate::app::App;
    use crate::core::New;
    use crate::request::Request;
    use crate::router::PathParams;
    use crate::server::ConnectionInfo;
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::test_client::TestClient;
    use crate::routes;

    struct AppConfig { version: &'static str }

    fn get_version(
        _req: &Request,
        _p: &PathParams,
        _c: &ConnectionInfo,
        state: &Arc<Container>,
    ) -> Response {
        let cfg = state.get::<AppConfig>().unwrap();
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![crate::range::Range::get_content_range(
            cfg.version.as_bytes().to_vec(),
            "text/plain".to_string(),
        )];
        r
    }

    let mut container = Container::new();
    container.register(AppConfig { version: "2.0" });

    let app = routes! {
        App::with_state(container.into_arc()),
        GET "/version" => get_version,
    };

    let client = TestClient::new(app);
    let resp = client.get("/version").send();
    assert_eq!(resp.status(), 200);
    let body = resp.body_text().to_string();
    assert_eq!(body, "2.0");
}
