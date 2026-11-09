use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::join_path;
use crate::app::App;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::url_for;
use crate::server::ConnectionInfo;
use crate::test_client::TestClient;

fn text(body: &str) -> Response {
    Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, body)
}

fn signed_in(request: &Request, _: &ConnectionInfo, _: &()) -> Result<(), Response> {
    match request.header("x-user") {
        Some(_) => Ok(()),
        None => Err(Response::text(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "sign in")),
    }
}

#[test]
fn join_path_handles_slashes() {
    assert_eq!("/", join_path("", ""));
    assert_eq!("/", join_path("/", "/"));
    assert_eq!("/admin", join_path("/admin", "/"));
    assert_eq!("/admin/posts", join_path("/admin/", "/posts"));
    assert_eq!("/posts", join_path("", "posts"));
}

#[test]
fn group_prefixes_routes_and_runs_guards() {
    let app = App::with_state(())
        .get("/", |_, _, _, _| text("home"))
        .group("/admin", |admin| admin.get("/", |_, _, _, _| text("dashboard")).guard(signed_in).get("/users/{id}", |_, p, _, _| text(p.get("id").unwrap())));
    let client = TestClient::new(app);
    assert_eq!("home", client.get("/").send().body_text());
    // The guard applies to routes added before it too.
    assert_eq!(401, client.get("/admin").send().status());
    assert_eq!(401, client.get("/admin/users/7").send().status());
    let ok = client.get("/admin/users/7").header("X-User", "ann").send();
    assert_eq!((200, "7"), (ok.status(), ok.body_text()));
}

#[test]
fn nested_groups_run_outer_guards_first() {
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (o1, o2) = (Arc::clone(&order), Arc::clone(&order));
    let app = App::with_state(()).group("/a", move |a| {
        a.guard(move |_, _, _| {
            o1.lock().unwrap().push("outer");
            Ok(())
        })
        .group("/b", move |b| {
            b.guard(move |_, _, _| {
                o2.lock().unwrap().push("inner");
                Err(Response::text(STATUS_CODE_REASON_PHRASE.n403_forbidden, "no"))
            })
            .get("/c", |_, _, _, _| text("c"))
        })
    });
    assert_eq!(403, TestClient::new(app).get("/a/b/c").send().status());
    assert_eq!(vec!["outer", "inner"], *order.lock().unwrap());
}

struct Count(Arc<AtomicUsize>);

impl crate::middleware::Middleware for Count {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn crate::application::Application) -> Result<Response, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let mut response = next.execute(request, connection)?;
        response.headers.push(crate::header::Header::new("X-Counted", "yes"));
        Ok(response)
    }
}

struct Fail;

impl crate::middleware::Middleware for Fail {
    fn handle(&self, _: &Request, _: &ConnectionInfo, _: &dyn crate::application::Application) -> Result<Response, String> {
        Err("layer failed".into())
    }
}

#[test]
fn group_middleware_wraps_only_its_routes() {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = App::with_state(())
        .get("/open", |_, _, _, _| text("open"))
        .group("/api", |api| api.wrap(Count(Arc::clone(&hits))).get("/x", |_, _, _, _| text("x")))
        .group("/broken", |b| b.wrap(Fail).get("/", |_, _, _, _| text("never")));
    let client = TestClient::new(app);
    let x = client.get("/api/x").send();
    assert_eq!(Some("yes"), x.header("x-counted"));
    assert_eq!(None, client.get("/open").send().header("x-counted"));
    assert_eq!(1, hits.load(Ordering::SeqCst));
    assert_eq!(500, client.get("/broken").send().status());
}

#[test]
fn resource_registers_conventional_routes_and_names() {
    let app = App::with_state(()).resource("/grp_posts", |r| {
        r.show(|_, p, _, _| text(&format!("show {}", p.get("id").unwrap())))
            .new_form(|_, _, _, _| text("new"))
            .index(|_, _, _, _| text("index"))
            .create(|_, _, _, _| text("create"))
            .edit(|_, p, _, _| text(&format!("edit {}", p.get("id").unwrap())))
            .update(|_, _, _, _| text("update"))
            .destroy(|_, _, _, _| text("destroy"))
            .extra(|g| g.post("/{id}/publish", |_, _, _, _| text("publish")))
    });
    assert!(app.route_conflicts().is_empty(), "{:?}", app.route_conflicts());
    let client = TestClient::new(app);
    assert_eq!("index", client.get("/grp_posts").send().body_text());
    assert_eq!("new", client.get("/grp_posts/new").send().body_text());
    assert_eq!("show 3", client.get("/grp_posts/3").send().body_text());
    assert_eq!("edit 3", client.get("/grp_posts/3/edit").send().body_text());
    assert_eq!("create", client.post("/grp_posts").send().body_text());
    assert_eq!("update", client.put("/grp_posts/3").send().body_text());
    assert_eq!("update", client.patch("/grp_posts/3").send().body_text());
    assert_eq!("destroy", client.delete("/grp_posts/3").send().body_text());
    assert_eq!("publish", client.post("/grp_posts/3/publish").send().body_text());
    assert_eq!(Some("/grp_posts/9/edit".to_string()), url_for("grp_posts.edit", &[("id", "9")]));
    assert_eq!(Some("/grp_posts".to_string()), url_for("grp_posts.index", &[]));
}

#[test]
fn resource_without_actions_registers_nothing() {
    let app = App::with_state(()).resource("/empty_res", |r| r);
    assert!(app.route_entries().is_empty());
}

#[test]
fn name_names_the_last_route() {
    let _app = App::with_state(()).get("/grp_users/{id}", |_, _, _, _| text("u")).name("grp_user").group("/grp_admin", |g| g.get("/{*rest}", |_, _, _, _| text("a")).name("grp_admin_any"));
    assert_eq!(Some("/grp_users/a%20b".to_string()), url_for("grp_user", &[("id", "a b")]));
    assert_eq!(Some("/grp_admin/x/y".to_string()), url_for("grp_admin_any", &[("rest", "x/y")]));
    assert_eq!(None, url_for("grp_user", &[]));
    assert_eq!(None, url_for("grp_no_such_route", &[]));
}

#[test]
fn typed_handlers_join_a_group() {
    struct Hello;
    impl crate::handler::RouteSpec for Hello {
        const METHOD: &'static str = "GET";
        const PATTERN: &'static str = "/hello";
    }
    impl crate::handler::Handler<()> for Hello {
        fn handle(&self, _: &Request, _: &crate::router::PathParams, _: &ConnectionInfo, _: &Arc<()>) -> Response {
            text("typed")
        }
    }
    let app = App::with_state(()).group("/v1", |g| g.route(Hello));
    assert_eq!("typed", TestClient::new(app).get("/v1/hello").send().body_text());
}

#[cfg(feature = "http2")]
#[test]
fn async_groups_and_resources() {
    let app = App::with_async_state(7u32)
        .group("/agrp", |g| {
            g.guard(|request, _, state: &u32| if *state == 7 && request.header("x-user").is_some() { Ok(()) } else { Err(Response::text(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "no")) })
                .get("/me", |_, _, _, state| async move { text(&format!("me {state}")) })
                .resource("/items", |r| r.index(|_, _, _, _| async { text("items") }).update(|_, p, _, _| async move { text(&format!("up {}", p.get("id").unwrap())) }))
        })
        .get("/agrp_open", |_, _, _, _| async { text("open") })
        .name("agrp_open");
    assert!(app.route_conflicts().is_empty());
    let client = TestClient::new(app);
    assert_eq!(401, client.get("/agrp/me").send().status());
    assert_eq!("me 7", client.get("/agrp/me").header("X-User", "a").send().body_text());
    assert_eq!("items", client.get("/agrp/items").header("X-User", "a").send().body_text());
    assert_eq!("up 4", client.patch("/agrp/items/4").header("X-User", "a").send().body_text());
    assert_eq!("open", client.get("/agrp_open").send().body_text());
    assert_eq!(Some("/agrp/items/4".to_string()), url_for("items.update", &[("id", "4")]));
    assert_eq!(Some("/agrp_open".to_string()), url_for("agrp_open", &[]));
}
