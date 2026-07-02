use std::sync::Arc;
use super::Container;

// ── Concrete type tests ────────────────────────────────────────────────────────

#[test]
fn register_and_get_concrete() {
    let mut c = Container::new();
    c.register(42u32);
    assert_eq!(*c.get::<u32>().unwrap(), 42);
}

#[test]
fn register_string_service() {
    let mut c = Container::new();
    c.register(String::from("hello"));
    assert_eq!(c.get::<String>().unwrap().as_str(), "hello");
}

#[test]
fn get_returns_none_for_unregistered_type() {
    let c = Container::new();
    assert!(c.get::<u32>().is_none());
    assert!(c.get::<String>().is_none());
}

#[test]
fn later_registration_replaces_earlier() {
    let mut c = Container::new();
    c.register(1u32);
    c.register(2u32);
    assert_eq!(*c.get::<u32>().unwrap(), 2);
}

#[test]
fn multiple_types_coexist_independently() {
    let mut c = Container::new();
    c.register(100u32)
     .register(200u64)
     .register(String::from("three"));

    assert_eq!(*c.get::<u32>().unwrap(), 100);
    assert_eq!(*c.get::<u64>().unwrap(), 200);
    assert_eq!(c.get::<String>().unwrap().as_str(), "three");
}

#[test]
fn get_returns_arc_with_correct_refcount() {
    let mut c = Container::new();
    c.register(99u32);

    let a1 = c.get::<u32>().unwrap();
    let a2 = c.get::<u32>().unwrap();
    // Both point to the same allocation
    assert!(Arc::ptr_eq(&a1, &a2));
    assert_eq!(Arc::strong_count(&a1), 3); // container + a1 + a2
}

#[test]
fn register_struct_service() {
    #[derive(PartialEq, Debug)]
    struct Config { port: u16, host: String }

    let mut c = Container::new();
    c.register(Config { port: 8080, host: "localhost".into() });

    let cfg = c.get::<Config>().unwrap();
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "localhost");
}

// ── provide (Arc registration) ─────────────────────────────────────────────────

#[test]
fn provide_arc_concrete() {
    let mut c = Container::new();
    c.provide(Arc::new(55u32));
    assert_eq!(*c.get::<u32>().unwrap(), 55);
}

// ── Trait object tests ─────────────────────────────────────────────────────────

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
fn provide_and_get_trait_object() {
    let mut c = Container::new();
    c.provide::<dyn Greeter>(Arc::new(Hello));

    let g = c.get::<dyn Greeter>().unwrap();
    assert_eq!(g.greet(), "hello");
}

#[test]
fn trait_object_method_dispatch_calls_correct_impl() {
    let mut c = Container::new();
    c.provide::<dyn Greeter>(Arc::new(Hi));

    assert_eq!(c.get::<dyn Greeter>().unwrap().greet(), "hi");
}

#[test]
fn provide_replaces_previous_trait_object() {
    let mut c = Container::new();
    c.provide::<dyn Greeter>(Arc::new(Hello));
    c.provide::<dyn Greeter>(Arc::new(Hi));

    assert_eq!(c.get::<dyn Greeter>().unwrap().greet(), "hi");
}

#[test]
fn concrete_and_trait_registrations_use_separate_keys() {
    // Registering Hello by concrete type and as dyn Greeter are independent.
    let mut c = Container::new();
    c.register(Hello);
    c.provide::<dyn Greeter>(Arc::new(Hi));

    // Concrete type Hello: registered as Hello
    assert!(c.get::<Hello>().is_some());
    // Trait object dyn Greeter: registered separately, resolves to Hi
    assert_eq!(c.get::<dyn Greeter>().unwrap().greet(), "hi");
}

// ── Named services ─────────────────────────────────────────────────────────────

#[test]
fn register_named_and_get_named() {
    let mut c = Container::new();
    c.register_named("primary", 5432u16)
     .register_named("replica", 5433u16);

    assert_eq!(*c.get_named::<u16>("primary").unwrap(), 5432);
    assert_eq!(*c.get_named::<u16>("replica").unwrap(), 5433);
}

#[test]
fn get_named_wrong_name_returns_none() {
    let mut c = Container::new();
    c.register_named("known", 1u32);

    assert!(c.get_named::<u32>("unknown").is_none());
}

#[test]
fn unnamed_and_named_of_same_type_are_independent() {
    let mut c = Container::new();
    c.register(0u32);
    c.register_named("special", 99u32);

    assert_eq!(*c.get::<u32>().unwrap(), 0);
    assert_eq!(*c.get_named::<u32>("special").unwrap(), 99);
}

#[test]
fn register_named_replaces_same_name() {
    let mut c = Container::new();
    c.register_named("db", 1u32);
    c.register_named("db", 2u32);

    assert_eq!(*c.get_named::<u32>("db").unwrap(), 2);
}

#[test]
fn provide_named_trait_object() {
    let mut c = Container::new();
    c.provide_named::<dyn Greeter>("en", Arc::new(Hello));
    c.provide_named::<dyn Greeter>("short", Arc::new(Hi));

    assert_eq!(c.get_named::<dyn Greeter>("en").unwrap().greet(), "hello");
    assert_eq!(c.get_named::<dyn Greeter>("short").unwrap().greet(), "hi");
}

// ── Inspection ─────────────────────────────────────────────────────────────────

#[test]
fn contains_returns_true_when_registered() {
    let mut c = Container::new();
    assert!(!c.contains::<u32>());
    c.register(1u32);
    assert!(c.contains::<u32>());
}

#[test]
fn contains_returns_false_for_other_types() {
    let mut c = Container::new();
    c.register(1u32);
    assert!(!c.contains::<u64>());
    assert!(!c.contains::<String>());
}

#[test]
fn contains_named_checks_name_and_type() {
    let mut c = Container::new();
    c.register_named("x", 1u32);

    assert!(c.contains_named::<u32>("x"));
    assert!(!c.contains_named::<u32>("y"));
    assert!(!c.contains_named::<u64>("x"));
}

#[test]
fn len_and_is_empty() {
    let mut c = Container::new();
    assert!(c.is_empty());
    assert_eq!(c.len(), 0);

    c.register(1u32);
    assert!(!c.is_empty());
    assert_eq!(c.len(), 1);

    c.register(2u64);
    assert_eq!(c.len(), 2);

    // Named services don't count in len (only unnamed)
    c.register_named("x", 3u32);
    assert_eq!(c.len(), 2);
}

// ── Arc sharing ────────────────────────────────────────────────────────────────

#[test]
fn into_arc_allows_sharing_across_threads() {
    let mut c = Container::new();
    c.register(42u32);
    let arc = c.into_arc();

    let arc2 = Arc::clone(&arc);
    let handle = std::thread::spawn(move || {
        *arc2.get::<u32>().unwrap()
    });
    assert_eq!(handle.join().unwrap(), 42);
}

#[test]
fn shared_arc_container_resolves_trait_objects_from_thread() {
    let mut c = Container::new();
    c.provide::<dyn Greeter>(Arc::new(Hello));
    let shared = c.into_arc();

    let shared2 = Arc::clone(&shared);
    let result = std::thread::spawn(move || {
        shared2.get::<dyn Greeter>().unwrap().greet().to_string()
    })
    .join()
    .unwrap();

    assert_eq!(result, "hello");
}

// ── Integration: use as AppWithState ──────────────────────────────────────────

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
