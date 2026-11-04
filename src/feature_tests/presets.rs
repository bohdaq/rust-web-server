//! The feature presets (`web`, `api`, `proxy`, `full`) and the docs.rs
//! feature lists, checked against the manifests (18.32.0, #96): a new feature
//! must join `full` and docs.rs, or be named here as left out on purpose.

use std::collections::BTreeMap;

const PRESETS: [&str; 4] = ["web", "api", "proxy", "full"];

/// Features `full` leaves out, and why (repeated in Cargo.toml).
const NOT_IN_FULL: [&str; 4] = [
    "fips",     // needs CMake and Go
    "pkcs11",   // only useful with an HSM
    "http1",    // the sync server mode, an alternative to http2
    "mimalloc", // the binary's allocator; nothing for a library
];

/// Features the facade's docs.rs build leaves out: no library items, and
/// `fips` doesn't build there.
const NOT_ON_DOCS_RS: [&str; 2] = ["fips", "mimalloc"];

/// The `[features]` table of a manifest: name → entries. Arrays may span
/// lines; `#` comments are dropped (no feature entry contains `#`).
fn features(manifest: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut in_table = false;
    let mut pending: Option<(String, String)> = None;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if let Some((name, mut acc)) = pending.take() {
            acc.push_str(line);
            if line.contains(']') {
                out.insert(name, entries(&acc));
            } else {
                pending = Some((name, acc));
            }
            continue;
        }
        if line.starts_with('[') {
            in_table = line == "[features]";
            continue;
        }
        if !in_table || line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            let (name, value) = (name.trim().to_string(), value.trim().to_string());
            if value.contains(']') {
                out.insert(name, entries(&value));
            } else {
                pending = Some((name, value));
            }
        }
    }
    out
}

/// `features = [...]` under `[package.metadata.docs.rs]`, if any.
fn docs_rs_features(manifest: &str) -> Option<Vec<String>> {
    let table = manifest.split("\n[package.metadata.docs.rs]\n").nth(1)?;
    let table = table.split("\n[").next().unwrap_or("");
    let line = table.lines().find(|l| l.trim_start().starts_with("features"))?;
    Some(entries(line.split_once('=')?.1))
}

/// `name` and every feature it turns on, transitively (`dep:` and
/// `crate/feature` entries are not features of this manifest).
fn closure<'a>(f: &'a BTreeMap<String, Vec<String>>, name: &'a str) -> Vec<&'a str> {
    let mut out = vec![name];
    let mut i = 0;
    while i < out.len() {
        for m in f.get(out[i]).into_iter().flatten() {
            if f.contains_key(m) && !out.contains(&m.as_str()) {
                out.push(m);
            }
        }
        i += 1;
    }
    out
}

fn entries(array: &str) -> Vec<String> {
    array
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_manifest_reader_handles_multi_line_arrays_and_comments() {
    let manifest = "[package]\nname = \"x\"\n[features]\n# note\na = [\"b\", # why\n  \"c\"]\nb = []  # empty\nc = [\"dep:z\"]\n[dependencies]\nz = \"1\"\n";
    let f = features(manifest);
    assert_eq!(f.keys().collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(f["a"], ["b", "c"]);
    assert!(f["b"].is_empty());
    assert_eq!(docs_rs_features(manifest), None, "no docs.rs table");
    let with = format!("{manifest}[package.metadata.docs.rs]\nfeatures = [\"a\"]\n[lib]\n");
    assert_eq!(docs_rs_features(&with), Some(vec!["a".to_string()]));
    let mentioned = format!("# see [package.metadata.docs.rs]\n{manifest}");
    assert_eq!(docs_rs_features(&mentioned), None, "a mention in a comment is not the table");
    assert_eq!(closure(&f, "a"), ["a", "b", "c"], "dep: entries are not features");
    assert_eq!(closure(&f, "missing"), ["missing"]);
}

#[test]
fn presets_only_turn_on_other_features() {
    let f = features(&read("Cargo.toml"));
    for preset in PRESETS {
        let members = f.get(preset).unwrap_or_else(|| panic!("no `{preset}` preset"));
        assert!(!members.is_empty(), "{preset} is empty");
        for m in members {
            assert!(f.contains_key(m), "{preset}: `{m}` is not a feature (presets only enable features)");
            assert!(!PRESETS.contains(&m.as_str()), "{preset}: includes the preset `{m}`");
            assert_ne!(m, "fips", "{preset}: fips needs CMake and Go");
        }
    }
}

#[test]
fn full_has_every_feature_but_the_named_exceptions() {
    let f = features(&read("Cargo.toml"));
    let full = closure(&f, "full");
    for name in f.keys() {
        if name == "default" || PRESETS.contains(&name.as_str()) {
            continue;
        }
        let left_out = NOT_IN_FULL.contains(&name.as_str());
        assert_eq!(full.contains(&name.as_str()), !left_out, "`{name}`: add it to `full`, or to NOT_IN_FULL with the reason in Cargo.toml");
    }
}

#[test]
fn docs_rs_documents_every_feature_that_has_items() {
    let manifest = read("Cargo.toml");
    let f = features(&manifest);
    let listed = docs_rs_features(&manifest).expect("[package.metadata.docs.rs] features");
    let on: Vec<&str> = listed.iter().flat_map(|name| closure(&f, name)).collect();
    for name in f.keys() {
        if name == "default" || PRESETS.contains(&name.as_str()) {
            continue;
        }
        assert_eq!(on.contains(&name.as_str()), !NOT_ON_DOCS_RS.contains(&name.as_str()), "docs.rs features and `{name}`");
    }
    assert!(manifest.contains("rustdoc-args = [\"--cfg\", \"docsrs\"]"));
}

#[test]
fn every_crate_documents_its_features_on_docs_rs() {
    let crates = ["rws-base", "rws-http", "rws-server", "rws-proxy", "rws-acme", "rws-gateway-api", "rws-storage", "rws-tower", "rws-wasm-plugin"];
    for name in crates {
        let manifest = read(&format!("crates/{name}/Cargo.toml"));
        assert!(manifest.contains("[package.metadata.docs.rs]"), "{name}: no docs.rs table");
        assert!(manifest.contains("rustdoc-args = [\"--cfg\", \"docsrs\"]"), "{name}: no --cfg docsrs");
        assert!(manifest.contains("'cfg(docsrs)'"), "{name}: cfg(docsrs) not declared (unexpected_cfgs is deny)");
        let lib = read(&format!("crates/{name}/src/lib.rs"));
        assert!(lib.contains("#![cfg_attr(docsrs, feature(doc_cfg))]"), "{name}: no doc_cfg");
        let listed = docs_rs_features(&manifest).unwrap_or_default();
        for feature in features(&manifest).keys() {
            // test-support: test helpers; fips: needs CMake and Go (or only labels --version).
            let skip = ["default", "test-support", "fips"].contains(&feature.as_str());
            assert_eq!(listed.contains(feature), !skip, "{name}: docs.rs features and `{feature}`");
        }
    }
    assert!(read("src/lib.rs").contains("#![cfg_attr(docsrs, feature(doc_cfg))]"));
}
