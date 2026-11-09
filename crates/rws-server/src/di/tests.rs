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


// ── Lifecycle (18.35.0) ──────────────────────────────────────────────────────

mod lifecycle {
    use std::sync::{Arc, Mutex};

    use crate::di::{Container, Lifecycle};

    struct Pool {
        log: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }

    impl Lifecycle for Pool {
        fn start(&self) -> Result<(), String> {
            self.log.lock().unwrap().push("pool start".into());
            if self.fail { Err("no database".into()) } else { Ok(()) }
        }
        fn stop(&self) {
            self.log.lock().unwrap().push("pool stop".into());
        }
    }

    struct Plain;
    impl Lifecycle for Plain {}

    fn container(fail: bool) -> (Container, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut c = Container::new();
        let (a, b, d) = (Arc::clone(&log), Arc::clone(&log), Arc::clone(&log));
        c.on_start("config", move |_| {
            a.lock().unwrap().push("config start".into());
            Ok(())
        })
        .on_stop("config", move |_| b.lock().unwrap().push("config stop".into()))
        .register_managed(Pool { log: Arc::clone(&log), fail })
        .on_stop("flush", move |c| d.lock().unwrap().push(format!("flush stop, pool registered: {}", c.contains::<Pool>())));
        (c, log)
    }

    #[test]
    fn start_in_order_stop_in_reverse() {
        let (c, log) = container(false);
        assert!(!c.is_started());
        c.start().unwrap();
        assert!(c.is_started());
        c.start().unwrap(); // already started: nothing runs
        c.stop();
        c.stop(); // already stopped: nothing runs
        assert!(!c.is_started());
        assert_eq!(
            vec!["config start", "pool start", "flush stop, pool registered: true", "pool stop", "config stop"],
            *log.lock().unwrap()
        );
    }

    #[test]
    fn a_failed_start_stops_what_started_before_it() {
        let (c, log) = container(true);
        let err = c.start().unwrap_err();
        assert!(err.ends_with("Pool: no database"), "{err}");
        assert!(!c.is_started());
        // The pool's own stop doesn't run (it never started); the config's does.
        assert_eq!(vec!["config start", "pool start", "config stop"], *log.lock().unwrap());
        c.stop();
        assert_eq!(3, log.lock().unwrap().len());
    }

    #[test]
    fn default_hooks_do_nothing_and_services_resolve() {
        let mut c = Container::new();
        c.register_managed(Plain);
        assert!(c.get::<Plain>().is_some());
        c.start().unwrap();
        c.stop();
        let empty = Container::new();
        empty.start().unwrap();
        assert!(empty.is_started());
    }
}
