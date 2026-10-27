use super::*;

#[test]
fn members_are_parsed_and_decoded() {
    let b = Baggage::parse(" userId=alice , serverNode = DF%2028 ;prop1;k=v, isProduction=false");
    assert_eq!(Some("alice"), b.get("userId"));
    assert_eq!(Some("DF 28"), b.get("serverNode"));
    assert_eq!(Some("false"), b.get("isProduction"));
    assert_eq!(3, b.len());
    assert_eq!("userId=alice,serverNode=DF%2028;prop1;k=v,isProduction=false", b.to_header(), "properties kept, value re-encoded");
}

#[test]
fn malformed_members_are_skipped() {
    let b = Baggage::parse("good=1,noequals,bad key=2,=empty,quote=\"x\",pct=%zz,utf=%C3%A9,,last=ok");
    let keys: Vec<&str> = b.iter().map(|(k, _)| k).collect();
    assert_eq!(vec!["good", "utf", "last"], keys);
    assert_eq!(Some("é"), b.get("utf"));
    assert!(Baggage::parse("").is_empty());
}

#[test]
fn limits_drop_the_excess() {
    let many: Vec<String> = (0..100).map(|i| format!("k{i}=v")).collect();
    assert_eq!(MAX_MEMBERS, Baggage::parse(&many.join(",")).len());
    let big = format!("a={},b=small", "x".repeat(MAX_BYTES));
    assert!(Baggage::parse(&big).is_empty(), "a member past 8192 bytes ends parsing");
    let mut b = Baggage::default();
    for i in 0..MAX_MEMBERS {
        assert!(b.set(&format!("k{i}"), "v"));
    }
    assert!(!b.set("one-more", "v"));
    assert!(b.set("k0", "replaced"), "replacing an existing key is fine at the cap");
}

#[test]
fn setting_encodes_on_the_way_out() {
    let mut b = Baggage::default();
    assert!(b.set("tenant", "a,b; c=\"d\" 100%"));
    assert!(!b.set("bad key", "x"));
    let header = b.to_header();
    assert_eq!("tenant=a%2Cb%3B%20c=%22d%22%20100%25", header);
    assert_eq!(Some("a,b; c=\"d\" 100%"), Baggage::parse(&header).get("tenant"), "round trip");
    b.remove("tenant");
    assert!(b.is_empty());
}
