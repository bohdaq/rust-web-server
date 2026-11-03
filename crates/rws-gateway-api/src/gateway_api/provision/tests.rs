use super::*;

fn gateway(namespace: &str, name: &str) -> Gateway {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "Gateway",
        "metadata": {"name": name, "namespace": namespace},
        "spec": {"gatewayClassName": "rws", "listeners": [], "infrastructure": {"labels": {"team": "a"}, "annotations": {"note": "x"}}}
    }))
    .unwrap()
}

#[test]
fn each_gateway_gets_a_deployment_of_its_own_pods() {
    let (a, b) = (gateway("apps", "web"), gateway("other", "api"));
    let settings = ProvisionSettings { image: "ghcr.io/bohdaq/rws:18.8.0".to_string(), service_account: "rws".to_string(), ..Default::default() };
    let desired = desired(&[&a, &b], "rws-system", "rws8.tech/gateway-controller", (10000, 10999), &settings);
    let web = &desired[&("apps".to_string(), "web".to_string())];
    assert_eq!((Some("rws-gw-apps-web"), Some("rws-system")), (web.metadata.name.as_deref(), web.metadata.namespace.as_deref()));
    let spec = web.spec.as_ref().unwrap();
    assert_eq!(Some(&pod_labels("apps", "web")), spec.selector.match_labels.as_ref());
    let template = spec.template.metadata.as_ref().unwrap();
    assert_eq!(Some("a"), template.labels.as_ref().unwrap().get("team").map(String::as_str), "infrastructure labels on the pods");
    let pod = spec.template.spec.as_ref().unwrap();
    assert_eq!(Some("rws"), pod.service_account_name.as_deref());
    let env: BTreeMap<String, String> = pod.containers[0].env.iter().flatten().filter_map(|e| Some((e.name.clone(), e.value.clone()?))).collect();
    assert_eq!(Some("apps/web"), env.get(super::super::RWS_CONFIG_GATEWAY_API_GATEWAY).map(String::as_str));
    assert_eq!(Some("10000-10999"), env.get(super::super::RWS_CONFIG_GATEWAY_API_PORT_RANGE).map(String::as_str));
    assert!(same(web, web));
    let mut changed = web.clone();
    changed.spec.as_mut().unwrap().template.spec.as_mut().unwrap().containers[0].image = Some("other".to_string());
    assert!(!same(web, &changed));

    // A Deployment whose Gateway is gone is stale; others aren't touched.
    let mut gone = web.clone();
    gone.metadata.name = Some("rws-gw-apps-gone".to_string());
    let mut foreign = web.clone();
    foreign.metadata.name = Some("unrelated".to_string());
    foreign.metadata.labels = Some(BTreeMap::new());
    let current = vec![web.clone(), gone, foreign];
    assert_eq!(vec![Some("rws-gw-apps-gone")], stale(&current, &desired).iter().map(|d| d.metadata.name.as_deref()).collect::<Vec<_>>());
    assert!(deployment_name("n".repeat(40).as_str(), "g".repeat(40).as_str()).len() <= 63);
}
