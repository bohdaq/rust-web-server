//! A Gateway's status addresses and their routability (Gateway API v1.7,
//! experimental, GEP-5093), from its Service: the load balancer's ingress
//! addresses and the Service's ClusterIPs.
//!
//! Each `spec.addresses` entry is matched on its own, by a distinct
//! address:
//! - routability `""` (or unset): a load-balancer address — rws's default
//!   scope, with no reachability promise beyond the load balancer's;
//! - `Cluster`: one of the Service's ClusterIPs (a requested value is asked
//!   of the Service as its `clusterIP`);
//! - `testing.x-k8s.io/sentinel` (reserved for conformance, no guarantee):
//!   like `""`, reported as asked;
//! - anything else: rws can't satisfy it.
//!
//! An entry with a value matches only that address. With `spec.addresses`
//! empty, every load-balancer address is reported, with routability `""`.

use super::types::{GatewaySpecAddress, GatewayStatusAddress};

/// The routability reserved for conformance tests.
pub const SENTINEL: &str = "testing.x-k8s.io/sentinel";
/// A ClusterIP.
pub const CLUSTER: &str = "Cluster";

/// The status addresses and the `AddressesAssigned` condition (status,
/// reason, message).
#[derive(Debug, PartialEq)]
pub struct Assignment {
    pub addresses: Vec<GatewayStatusAddress>,
    pub condition: (bool, &'static str, String),
}

/// Whether rws can give an entry's routability at all.
pub fn routability_supported(routability: &str) -> bool {
    matches!(routability, "" | CLUSTER | SENTINEL)
}

/// The address a `Cluster` entry asks to be the Service's `clusterIP`.
pub fn requested_cluster_ip(spec: &[GatewaySpecAddress]) -> Option<&str> {
    spec.iter().find(|a| a.routability.as_deref() == Some(CLUSTER) && !a.value.is_empty() && is_ip(a)).map(|a| a.value.as_str())
}

fn is_ip(address: &GatewaySpecAddress) -> bool {
    address.address_type.as_deref().unwrap_or("IPAddress") == "IPAddress"
}

/// Match `spec` against the load-balancer addresses and ClusterIPs.
pub fn assign(spec: &[GatewaySpecAddress], load_balancer: &[GatewayStatusAddress], cluster_ips: &[String]) -> Assignment {
    if spec.is_empty() {
        let addresses = load_balancer.iter().map(|a| GatewayStatusAddress { routability: Some(String::new()), ..a.clone() }).collect();
        return Assignment { addresses, condition: (true, "AddressesAssigned", "All addresses are assigned".to_string()) };
    }
    let mut used: Vec<String> = Vec::new();
    let mut addresses = Vec::new();
    let mut unsatisfied = Vec::new();
    for entry in spec {
        let routability = entry.routability.clone().unwrap_or_default();
        let candidates: Vec<(Option<String>, String)> = match routability.as_str() {
            _ if !is_ip(entry) => Vec::new(),
            "" | SENTINEL => load_balancer.iter().map(|a| (a.address_type.clone(), a.value.clone())).collect(),
            CLUSTER => cluster_ips.iter().map(|ip| (Some("IPAddress".to_string()), ip.clone())).collect(),
            _ => Vec::new(),
        };
        let found = candidates.into_iter().find(|(_, value)| !used.contains(value) && (entry.value.is_empty() || *value == entry.value));
        match found {
            Some((address_type, value)) => {
                used.push(value.clone());
                addresses.push(GatewayStatusAddress { address_type, value, routability: Some(routability) });
            }
            None => unsatisfied.push(describe(entry)),
        }
    }
    let condition = if unsatisfied.is_empty() {
        (true, "AddressesAssigned", "All addresses are assigned".to_string())
    } else if addresses.is_empty() {
        (false, "NotAssigned", format!("Not assigned: {}", unsatisfied.join(", ")))
    } else {
        (false, "PartiallyAssigned", format!("Not assigned: {}", unsatisfied.join(", ")))
    };
    Assignment { addresses, condition }
}

fn describe(entry: &GatewaySpecAddress) -> String {
    let value = if entry.value.is_empty() { "any address" } else { entry.value.as_str() };
    match entry.routability.as_deref().unwrap_or("") {
        "" => value.to_string(),
        routability => format!("{value} ({routability})"),
    }
}

#[cfg(test)]
mod tests;
