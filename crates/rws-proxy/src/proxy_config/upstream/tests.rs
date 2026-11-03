use super::*;
use crate::header::Header;
use crate::proxy_config::DynamicProxy;
use crate::request::Request;

fn backends() -> Vec<String> {
    vec!["http://a:1".to_string(), "http://b:1".to_string(), "http://c:1".to_string()]
}

fn policy(weights: &[u32], sticky: bool, breaker: Option<BreakerConfig>, retry: Option<RetryConfig>) -> Arc<UpstreamPolicy> {
    let live = Arc::new(RwLock::new(backends()));
    let sticky = sticky.then(|| StickyConfig { cookie: "lb".to_string(), max_age_secs: None });
    Arc::new(UpstreamPolicy::new("api", live, &backends(), weights, sticky, breaker.as_ref(), retry))
}

fn proxy(policy: Arc<UpstreamPolicy>) -> DynamicProxy {
    let live = Arc::clone(&policy.live);
    DynamicProxy::new(live, 100, 100, None, None, false, "round_robin".to_string()).with_policy("api", Some(policy))
}

fn request(cookie: Option<&str>) -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: "/".to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: cookie.map(|c| vec![Header { name: "Cookie".into(), value: c.to_string().into() }]).unwrap_or_default(),
        body: Default::default(),
    }
}

#[test]
fn smooth_weighted_round_robin_interleaves() {
    let policy = policy(&[3, 1, 1], false, None, None);
    let picks: Vec<String> = (0..10).map(|_| policy.weighted_round_robin(&backends()).unwrap()).collect();
    let short: Vec<&str> = picks.iter().map(|b| &b[7..8]).collect();
    assert_eq!(vec!["a", "b", "a", "c", "a", "a", "b", "a", "c", "a"], short, "3:1:1, spread out (a b a c a per cycle)");
    assert_eq!(6, short.iter().filter(|b| **b == "a").count());
}

#[test]
fn weighted_random_follows_the_weights() {
    let policy = policy(&[2, 1, 1], false, None, None);
    let counts = (0..400).fold([0; 3], |mut counts, roll| {
        counts[policy.weighted_index(&backends(), roll)] += 1;
        counts
    });
    assert_eq!([200, 100, 100], counts, "every roll mod 4 maps to a, a, b, c");
}

#[test]
fn backend_ids_are_stable_and_opaque() {
    assert_eq!(backend_id("http://a:1"), backend_id("http://a:1"));
    assert_ne!(backend_id("http://a:1"), backend_id("http://b:1"));
    assert_eq!(12, backend_id("http://a:1").len());
    assert!(!backend_id("http://10.0.0.5:8080").contains("10.0"));
}

#[test]
fn cookies_are_read_by_name() {
    assert_eq!(Some("x".to_string()), cookie_value(&request(Some("a=1; lb=x; c=3")), "lb"));
    assert_eq!(None, cookie_value(&request(Some("lbx=1")), "lb"));
    assert_eq!(None, cookie_value(&request(None), "lb"));
}

#[test]
fn only_idempotent_buffered_requests_are_replayed() {
    let mut post = request(None);
    assert!(replayable(&post));
    post.method = "POST".to_string();
    assert!(!replayable(&post));
    let mut streamed = request(None);
    streamed.body = crate::request::Body::stream(std::io::empty());
    assert!(!replayable(&streamed));
}

#[test]
fn a_sticky_client_keeps_its_backend() {
    let proxy = proxy(policy(&[], true, None, None));
    let id = backend_id("http://c:1");
    for _ in 0..5 {
        assert_eq!("http://c:1", proxy.next_backend(&request(Some(&format!("lb={id}"))), "1.2.3.4", &[]).unwrap());
    }
    // An unknown id (a removed backend) falls back to the strategy.
    assert!(proxy.next_backend(&request(Some("lb=nope")), "1.2.3.4", &[]).is_ok());
    // Not when it was already tried (a retry goes elsewhere).
    assert_ne!("http://c:1", proxy.next_backend(&request(Some(&format!("lb={id}"))), "1.2.3.4", &["http://c:1".to_string()]).unwrap());
}

#[test]
fn the_breaker_takes_a_failing_backend_out_and_lets_it_back() {
    let breaker = BreakerConfig { failure_threshold: 2, recovery_secs: 0, half_open_probes: 1 };
    let policy = policy(&[], false, Some(breaker), None);
    policy.record("http://a:1", true);
    assert!(policy.admits("http://a:1"), "one failure isn't enough");
    policy.record("http://a:1", true);
    // Open; recovery 0 s: the next request is the probe, then it's closed again on success.
    assert!(policy.admits("http://a:1"));
    assert!(!policy.admits("http://a:1"), "only one probe at a time");
    policy.record("http://a:1", false);
    assert!(policy.admits("http://a:1"));
}

#[test]
fn when_every_backend_is_out_the_answer_is_503() {
    let breaker = BreakerConfig { failure_threshold: 1, recovery_secs: 3600, half_open_probes: 1 };
    let policy = policy(&[], false, Some(breaker), None);
    for backend in backends() {
        policy.record(&backend, true);
    }
    let proxy = proxy(policy);
    assert_eq!(Err(true), proxy.next_backend(&request(None), "1.2.3.4", &[]));
}

#[test]
fn metrics_name_the_upstream_and_backend() {
    let policy = policy(&[], false, Some(BreakerConfig { failure_threshold: 1, recovery_secs: 3600, half_open_probes: 1 }), None);
    policy.record("http://b:1", true);
    register(vec![Arc::clone(&policy)]);
    observe("metrics-test", "http://a:1", Some(200), Instant::now());
    observe("metrics-test", "http://a:1", None, Instant::now());
    observe_retry("metrics-test");
    let text = prometheus_text();
    assert!(text.contains(r#"rws_upstream_requests_total{upstream="metrics-test",backend="http://a:1",code="200"} 1"#), "{text}");
    assert!(text.contains(r#"rws_upstream_requests_total{upstream="metrics-test",backend="http://a:1",code="error"} 1"#), "{text}");
    assert!(text.contains(r#"rws_upstream_request_duration_seconds_count{upstream="metrics-test",backend="http://a:1"} 2"#), "{text}");
    assert!(text.contains(r#"rws_upstream_retries_total{upstream="metrics-test"} 1"#), "{text}");
    assert!(text.contains(r#"rws_upstream_backend_up{upstream="api",backend="http://b:1"} 0"#), "{text}");
    assert!(text.contains(r#"rws_upstream_backend_up{upstream="api",backend="http://a:1"} 1"#), "{text}");
    assert!(text.contains(r#"rws_upstream_circuit_breaker_state{upstream="api",backend="http://b:1"} 2"#), "{text}");
}

#[test]
fn the_config_is_parsed_and_checked() {
    let toml = r#"
[[upstream]]
name = "api"
backends = ["http://127.0.0.1:9001", "http://127.0.0.1:9002"]
weights = [3, 1]
[upstream.sticky]
cookie = "srv"
max_age_secs = 600
[upstream.circuit_breaker]
failure_threshold = 2
recovery_secs = 5
[upstream.retry]
attempts = 1
"#;
    let config = crate::proxy_config::ProxyConfig::from_str(toml);
    let upstream = &config.upstreams[0];
    assert_eq!(vec![3, 1], upstream.weights);
    assert_eq!(Some(StickyConfig { cookie: "srv".to_string(), max_age_secs: Some(600) }), upstream.sticky);
    assert_eq!(Some(BreakerConfig { failure_threshold: 2, recovery_secs: 5, half_open_probes: 1 }), upstream.circuit_breaker);
    assert_eq!(Some(RetryConfig { attempts: 1, status: vec![502, 503, 504] }), upstream.retry);
    let mut report = crate::config_check::ConfigReport::default();
    config.validate(&mut report);
    crate::proxy_config::validate::check_unknown_keys(toml, &mut report);
    assert!(report.errors.is_empty() && report.warnings.is_empty(), "{report:?}");

    let bad = toml.replace("weights = [3, 1]", "weights = [3, 0, 1]").replace("cookie = \"srv\"", "cookie = \"a b\"").replace("attempts = 1", "attempts = 1\nstatus = [200]");
    let mut report = crate::config_check::ConfigReport::default();
    crate::proxy_config::ProxyConfig::from_str(&bad).validate(&mut report);
    let errors = report.errors.join("\n");
    assert!(errors.contains("3 entries for 2 backends") && errors.contains("1 or more"), "{errors}");
    assert!(errors.contains("isn't a valid cookie name") && errors.contains("retry.status 200"), "{errors}");
}
