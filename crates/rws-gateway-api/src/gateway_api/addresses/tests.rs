use super::*;

fn spec(value: &str, routability: Option<&str>) -> GatewaySpecAddress {
    GatewaySpecAddress { address_type: None, value: value.to_string(), routability: routability.map(str::to_string) }
}

fn lb(value: &str) -> GatewayStatusAddress {
    GatewayStatusAddress { address_type: Some("IPAddress".to_string()), value: value.to_string(), routability: None }
}

fn reported(a: &Assignment) -> Vec<(String, String)> {
    a.addresses.iter().map(|x| (x.value.clone(), x.routability.clone().unwrap())).collect()
}

#[test]
fn without_spec_addresses_every_load_balancer_address_is_reported_as_default() {
    let a = assign(&[], &[lb("172.18.0.10")], &["10.96.0.5".to_string()]);
    assert_eq!(vec![("172.18.0.10".to_string(), String::new())], reported(&a));
    assert_eq!((true, "AddressesAssigned"), (a.condition.0, a.condition.1));
}

#[test]
fn each_entry_is_matched_by_its_own_scope() {
    let lbs = [lb("172.18.0.10")];
    let cluster = ["10.96.0.5".to_string()];
    let a = assign(&[spec("", Some("Cluster")), spec("", None)], &lbs, &cluster);
    assert_eq!(vec![("10.96.0.5".to_string(), "Cluster".to_string()), ("172.18.0.10".to_string(), String::new())], reported(&a));
    assert!(a.condition.0);
    let sentinel = assign(&[spec("", Some(SENTINEL))], &lbs, &cluster);
    assert_eq!(vec![("172.18.0.10".to_string(), SENTINEL.to_string())], reported(&sentinel), "reported as asked");
}

#[test]
fn values_must_match_and_addresses_are_not_shared_between_entries() {
    let lbs = [lb("172.18.0.10")];
    let a = assign(&[spec("172.18.0.10", None), spec("", None)], &lbs, &[]);
    assert_eq!(1, a.addresses.len(), "one load-balancer address, one entry");
    assert_eq!((false, "PartiallyAssigned"), (a.condition.0, a.condition.1));
    let wrong = assign(&[spec("10.96.9.9", Some("Cluster"))], &lbs, &["10.96.0.5".to_string()]);
    assert!(wrong.addresses.is_empty());
    assert_eq!((false, "NotAssigned"), (wrong.condition.0, wrong.condition.1));
}

#[test]
fn unknown_scopes_are_never_substituted() {
    let a = assign(&[spec("", Some("example.com/CorpWan")), spec("", Some("testing.x-k8s.io/other"))], &[lb("172.18.0.10")], &["10.96.0.5".to_string()]);
    assert!(a.addresses.is_empty());
    assert_eq!("NotAssigned", a.condition.1);
    assert!(a.condition.2.contains("example.com/CorpWan"));
    assert!(!routability_supported("example.com/CorpWan"));
    assert!(routability_supported("") && routability_supported(CLUSTER) && routability_supported(SENTINEL));
}

#[test]
fn a_cluster_entry_with_a_value_asks_for_that_cluster_ip() {
    assert_eq!(Some("10.96.0.42"), requested_cluster_ip(&[spec("172.18.0.10", None), spec("10.96.0.42", Some("Cluster"))]));
    assert_eq!(None, requested_cluster_ip(&[spec("", Some("Cluster"))]));
}
