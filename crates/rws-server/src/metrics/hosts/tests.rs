use super::*;

fn of(host: &str) -> HostCount {
    snapshot().0.into_iter().find(|(h, _)| h == host).map(|(_, c)| c).unwrap_or_default()
}

#[test]
fn requests_are_counted_per_host_whatever_its_case() {
    let before = of("hosts-case.example");
    record(Some("hosts-case.example"), 200);
    record(Some("Hosts-Case.Example"), 503);
    record(Some(" hosts-case.example "), 404);
    let after = of("hosts-case.example");
    assert_eq!(after.requests - before.requests, 3);
    assert_eq!(after.errors - before.errors, 1, "only 5xx is an error");
}

#[test]
fn a_request_without_a_host_is_listed_under_the_empty_name() {
    let before = of("");
    record(None, 200);
    assert!(of("").requests > before.requests);
}

#[test]
fn made_up_hosts_take_each_others_place_not_a_busy_hosts() {
    let mut shard = Shard::default();
    shard.record(Some("shop.example"), 200);
    shard.record(Some("shop.example"), 200);
    for i in 0..MAX_HOSTS + 50 {
        shard.record(Some(&format!("made-up-{i}.example")), 200);
    }
    shard.record(Some(&"x".repeat(MAX_NAME + 1)), 500);
    assert_eq!(shard.hosts.len(), MAX_HOSTS);
    assert_eq!(shard.other, HostCount { requests: 52, errors: 1 }, "51 names lost their place, one was too long");
    assert_eq!(shard.hosts["shop.example"].requests, 2);
    // A host that turns up after the table filled gets a name and keeps it.
    shard.record(Some("late.example"), 200);
    shard.record(Some("late.example"), 502);
    for i in 0..MAX_HOSTS * 2 {
        shard.record(Some(&format!("more-{i}.example")), 200);
    }
    assert_eq!(shard.hosts["late.example"], HostCount { requests: 2, errors: 1 });
    assert_eq!(shard.hosts["shop.example"].requests, 2);
    let named: u64 = shard.hosts.values().map(|c| c.requests).sum();
    assert_eq!(named + shard.other.requests, 2 + MAX_HOSTS as u64 + 50 + 1 + 2 + MAX_HOSTS as u64 * 2, "every request is counted once");
}

#[test]
fn shards_merge_busiest_first_and_keep_the_cap() {
    let mut all = Shard::default();
    for s in 0..3 {
        let mut shard = Shard::default();
        shard.record(Some("busy.example"), 200);
        shard.record(Some("busy.example"), 502);
        // One more name than the shard has room for.
        for i in 0..MAX_HOSTS {
            shard.record(Some(&format!("s{s}-h{i}.example")), 200);
        }
        assert_eq!(shard.other.requests, 1);
        all.merge(&shard);
    }
    let (hosts, other) = all.listed();
    assert_eq!(hosts.len(), MAX_HOSTS);
    assert_eq!(hosts[0], ("busy.example".to_string(), HostCount { requests: 6, errors: 3 }));
    assert_eq!(hosts[1].0, "s0-h0.example", "equally busy hosts are listed by name");
    let listed: u64 = hosts.iter().map(|(_, c)| c.requests).sum();
    assert_eq!(listed + other.requests, 3 * (MAX_HOSTS as u64 + 2), "every request is counted once");
}
