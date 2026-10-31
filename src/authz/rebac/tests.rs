use super::*;

fn docs_schema() -> Schema {
    Schema::new()
        .relation("group", "member", "user | group#member").unwrap()
        .relation("folder", "owner", "user").unwrap()
        .relation("folder", "viewer", "user | group#member | owner").unwrap()
        .relation("doc", "parent", "folder").unwrap()
        .relation("doc", "owner", "user").unwrap()
        .relation("doc", "editor", "user | group#member | owner").unwrap()
        .relation("doc", "viewer", "user | group#member | editor | parent->viewer").unwrap()
}

fn authz(tuples: &[&str]) -> Authorizer {
    let authz = Authorizer::new(docs_schema(), MemoryStore::new()).unwrap();
    for t in tuples {
        authz.write_str(t).unwrap();
    }
    authz
}

#[test]
fn tuples_parse_and_print_back() {
    for text in ["doc:42#editor@user:alice", "doc:42#viewer@group:eng#member", "folder:a-b.c#owner@user:x_1", "doc:p/q#viewer@user:a.b@c"] {
        match Tuple::parse(text) {
            Ok(t) => assert_eq!(t.to_string(), text),
            Err(e) => assert_eq!(text, "doc:p/q#viewer@user:a.b@c", "{e}"),
        }
    }
    for bad in ["", "doc:42#editor", "doc#editor@user:a", "doc:#editor@user:a", "doc:42#@user:a", "doc:42#ed itor@user:a", "doc:42#editor@user", "d oc:1#r@user:a", "doc:4\n2#r@user:a"] {
        assert!(Tuple::parse(bad).is_err(), "{bad:?}");
    }
    assert!(Tuple::parse(&format!("doc:{}#r@user:a", "x".repeat(MAX_NAME + 1))).is_err());
    assert_eq!(Subject::parse("group:eng#member").unwrap().relation.as_deref(), Some("member"));
}

#[test]
fn expressions_parse_into_terms() {
    assert_eq!(
        parse_expression("user | group#member | editor | parent->viewer").unwrap(),
        [
            Term::Direct("user".into()),
            Term::DirectSet("group".into(), "member".into()),
            Term::Direct("editor".into()),
            Term::TupleToUserset { tupleset: "parent".into(), computed: "viewer".into() },
        ]
    );
    for bad in ["", "user ||", "user | ", "->viewer", "group#", "us er", "a->b->c"] {
        assert!(parse_expression(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_schema_resolves_names_and_rejects_dangling_references() {
    let schema = docs_schema().validate().unwrap();
    assert_eq!(schema.types(), ["doc", "folder", "group", "user"]);
    assert_eq!(schema.relations("doc"), ["editor", "owner", "parent", "viewer"]);
    assert!(schema.terms("doc", "viewer").unwrap().contains(&Term::Computed("editor".into())), "a bare relation name");
    let error = |s: Schema| s.validate().unwrap_err().to_string();
    assert!(error(Schema::new().relation("doc", "viewer", "team#member").unwrap()).contains("team#member isn't defined"));
    assert!(error(Schema::new().relation("doc", "viewer", "parent->viewer").unwrap()).contains("parent isn't a relation"));
    assert!(error(Schema::new().relation("folder", "x", "user").unwrap().relation("doc", "parent", "folder").unwrap().relation("doc", "viewer", "parent->viewer").unwrap())
        .contains("needs viewer on folder"));
    assert!(error(Schema::new().relation("group", "member", "user").unwrap().relation("doc", "parent", "group#member").unwrap().relation("doc", "viewer", "parent->member").unwrap())
        .contains("must name objects directly"));
}

#[test]
fn writes_must_fit_the_schema() {
    let authz = authz(&[]);
    assert!(authz.write_str("doc:1#owner@user:alice").unwrap());
    assert!(!authz.write_str("doc:1#owner@user:alice").unwrap(), "already there");
    assert!(authz.write_str("doc:1#editor@group:eng#member").unwrap());
    for bad in ["doc:1#owner@group:eng#member", "doc:1#viewer@folder:a", "doc:1#nope@user:a", "page:1#owner@user:a"] {
        assert!(matches!(authz.write_str(bad), Err(AuthzError::Schema(_))), "{bad}");
    }
    assert!(authz.delete(&Tuple::parse("doc:1#owner@user:alice").unwrap()).unwrap());
    assert!(!authz.delete(&Tuple::parse("doc:1#owner@user:alice").unwrap()).unwrap());
}

#[test]
fn checks_follow_direct_computed_set_and_parent_relations() {
    let authz = authz(&[
        "doc:42#owner@user:olga",
        "doc:42#editor@user:ed",
        "group:eng#member@user:gina",
        "group:all#member@group:eng#member",
        "doc:42#editor@group:all#member",
        "folder:specs#viewer@user:vic",
        "doc:42#parent@folder:specs",
        "folder:specs#owner@user:fran",
    ]);
    for (who, relation, allowed) in [
        ("user:olga", "viewer", true),  // owner → editor → viewer
        ("user:ed", "viewer", true),
        ("user:ed", "owner", false),
        ("user:gina", "editor", true),  // eng ⊂ all, all members edit
        ("user:vic", "viewer", true),   // via the folder
        ("user:fran", "viewer", true),  // folder owner → folder viewer → doc viewer
        ("user:fran", "editor", false),
        ("user:eve", "viewer", false),
        ("group:all#member", "editor", true),
    ] {
        assert_eq!(authz.check(who, relation, "doc:42").unwrap(), allowed, "{who} {relation}");
    }
    assert_eq!(
        authz.explain("user:gina", "editor", "doc:42").unwrap().unwrap(),
        ["doc:42#editor", "group:all#member", "group:eng#member", "group:eng#member@user:gina"]
    );
    assert_eq!(authz.explain("user:eve", "viewer", "doc:42").unwrap(), None);
    assert!(matches!(authz.check("user:a", "nope", "doc:42"), Err(AuthzError::Schema(_))));
    assert!(matches!(authz.check("nobody", "viewer", "doc:42"), Err(AuthzError::Parse(_))));
}

#[test]
fn cycles_end_and_long_chains_hit_the_depth_limit() {
    let authz = authz(&["group:a#member@group:b#member", "group:b#member@group:a#member", "group:a#member@user:x"]);
    assert!(authz.check("user:x", "member", "group:b").unwrap());
    assert!(!authz.check("user:y", "member", "group:b").unwrap(), "a cycle ends with a no");

    let chain = Authorizer::new(docs_schema(), MemoryStore::new()).unwrap();
    for i in 0..(MAX_DEPTH + 5) {
        chain.write_str(&format!("group:g{i}#member@group:g{}#member", i + 1)).unwrap();
    }
    chain.write_str(&format!("group:g{}#member@user:deep", MAX_DEPTH + 5)).unwrap();
    assert!(matches!(chain.check("user:deep", "member", "group:g0"), Err(AuthzError::Limit(_))));
    assert!(chain.check("user:deep", "member", &format!("group:g{}", 10)).unwrap(), "within the limit");
}

#[test]
fn wide_graphs_hit_the_read_budget() {
    let authz = Authorizer::new(docs_schema(), MemoryStore::new()).unwrap();
    for i in 0..(MAX_READS + 10) {
        authz.write_str(&format!("group:top#member@group:sub{i}#member")).unwrap();
    }
    assert!(matches!(authz.check("user:x", "member", "group:top"), Err(AuthzError::Limit(_))));
}

#[test]
fn memory_store_lists_by_filter() {
    let authz = authz(&["doc:1#owner@user:a", "doc:2#owner@user:a", "doc:2#editor@user:b", "folder:f#owner@user:a"]);
    let all = authz.store().list(&TupleFilter::default(), 100).unwrap();
    assert_eq!(all.len(), 4);
    let owned_by_a = TupleFilter { subject: Some(Subject::parse("user:a").unwrap()), object_kind: Some("doc".into()), ..Default::default() };
    assert_eq!(authz.store().list(&owned_by_a, 100).unwrap().iter().map(ToString::to_string).collect::<Vec<_>>(), ["doc:1#owner@user:a", "doc:2#owner@user:a"]);
    assert_eq!(authz.store().list(&TupleFilter::default(), 2).unwrap().len(), 2);
}
