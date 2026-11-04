use super::*;
use crate::test_env::TempPath;

fn parse(text: &str) -> AppManifest {
    AppManifest::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

fn error(text: &str) -> ManifestError {
    AppManifest::parse(text).expect_err("should be refused")
}

#[test]
fn empty_manifest_is_the_conventional_layout() {
    let manifest = parse("");
    assert_eq!(manifest, AppManifest::default());
    assert_eq!(manifest.paths.routes, "src/routes");
    assert_eq!(manifest.paths.static_files, "static");
    assert_eq!(manifest.paths.get("migrations"), Some("migrations"));
    assert_eq!(manifest.paths.get("nope"), None);
    assert_eq!(AppManifest::conventional("x").name, "x");
    let roles: Vec<&str> = manifest.paths.entries().iter().map(|(r, _)| *r).collect();
    assert_eq!(roles, ROLES);
}

#[test]
fn every_key_is_read_and_comments_are_ignored() {
    let manifest = parse(
        "\u{feff}# the app\n[app]\nname = \"todos\" # inline comment\n\n[paths]\nmain = 'src/bin/server.rs'\nlib = \"src/app.rs\"\n\
         routes = \"src/http/\"\nmodels = \"./src/db\"\njobs = \"src/work\"\nmigrations = \"db/migrations\"\n\
         templates = \"views\"\ntests = \"it\"\nstatic = \"frontend/dist # not a comment\"\n",
    );
    assert_eq!(manifest.name, "todos");
    let p = &manifest.paths;
    assert_eq!((p.main.as_str(), p.lib.as_str()), ("src/bin/server.rs", "src/app.rs"));
    assert_eq!((p.routes.as_str(), p.models.as_str(), p.jobs.as_str()), ("src/http", "src/db", "src/work"));
    assert_eq!((p.migrations.as_str(), p.templates.as_str(), p.tests.as_str()), ("db/migrations", "views", "it"));
    assert_eq!(p.static_files, "frontend/dist # not a comment");
    assert!(manifest.warnings.is_empty(), "{:?}", manifest.warnings);
}

#[test]
fn only_what_differs_is_written() {
    let manifest: AppManifest = "[paths]\nstatic = \"public\"".parse().unwrap();
    assert_eq!(manifest.paths.static_files, "public");
    assert_eq!(manifest.paths.models, "src/models");
    assert_eq!(manifest.name, "");
}

#[test]
fn top_level_and_dotted_keys_work() {
    let manifest = parse("name = \"blog\"\npaths.routes = \"src/web\"\n");
    assert_eq!(manifest.name, "blog");
    assert_eq!(manifest.paths.routes, "src/web");
}

#[test]
fn unknown_keys_and_tables_are_warnings() {
    let manifest = parse("[app]\nname = \"a\"\nedition = \"x\"\n[paths]\nassets = \"a\"\n[deploy]\nregion = \"eu\"\nfoo.bar = \"1\"\n");
    assert_eq!(manifest.name, "a");
    assert_eq!(manifest.warnings.len(), 3, "{:?}", manifest.warnings);
    assert!(manifest.warnings[0].contains("line 3") && manifest.warnings[0].contains("edition"));
    assert!(manifest.warnings[1].contains("assets") && manifest.warnings[1].contains("routes"));
    assert!(manifest.warnings[2].contains("[deploy]"));
}

#[test]
fn paths_must_stay_inside_the_project() {
    for (text, says) in [
        ("[paths]\nroutes = \"/etc\"", "absolute"),
        ("[paths]\nroutes = \"C:\\\\app\"", "absolute"),
        ("[paths]\nroutes = \"\\\\server\\\\share\"", "absolute"),
        ("[paths]\nmodels = \"src/../../x\"", "leaves the project"),
        ("[paths]\nmodels = \"..\"", "leaves the project"),
        ("[paths]\nstatic = \"  \"", "empty"),
        ("[paths]\nstatic = \"/\"", "empty"),
    ] {
        let e = error(text);
        assert_eq!(e.line, 2, "{text}");
        assert!(e.message.contains(says), "{text}: {e}");
    }
}

#[test]
fn malformed_lines_name_the_line_and_the_fix() {
    let e = error("[app]\nname = todos");
    assert_eq!(e.line, 2);
    assert!(e.message.contains("quoted string"), "{e}");
    assert!(e.to_string().starts_with("rws.app.toml line 2:"), "{e}");

    assert!(error("[app]\nname = \"todos").message.contains("quoted string"));
    assert!(error("name = \"a\" \"b\"").message.contains("quoted string"));
    assert!(error("name = \"a\\\"").message.contains("quoted string"), "an escaped quote doesn't close");
    assert!(error("[app\nname = \"a\"").message.contains("table header"));
    assert!(error("[[paths]]").message.contains("array tables"));
    assert!(error("just words").message.contains("key = "));
    assert!(error(" = \"x\"").message.contains("key is missing"));
    assert!(error("name = \"a\"\n[app]\nname = \"b\"").message.contains("twice"));
    assert!(error("name = \"  \"").message.contains("empty"));
    assert!(error("name = \"a\0b\"").message.contains("NUL"));
}

#[test]
fn oversized_input_is_refused_without_parsing() {
    let big = "# x\n".repeat(MAX_MANIFEST_SIZE / 4 + 1);
    let e = error(&big);
    assert_eq!(e.line, 0);
    assert!(e.to_string().starts_with("rws.app.toml: larger than"), "{e}");
}

#[test]
fn arbitrary_bytes_never_panic() {
    for text in ["[", "]", "=", "\"", "'", "[]", "a=", "a='", "[paths]\nroutes=''", "é = \"ü\"", "paths. = \"x\"", ".x = \"y\""] {
        let _ = AppManifest::parse(text);
    }
}

#[test]
fn load_without_a_manifest_is_conventional_named_after_the_package() {
    let dir = TempPath::new("app_manifest_absent");
    std::fs::create_dir_all(dir.join("src/routes")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[workspace]\nname = \"no\"\n[package]\nname = \"todo-api\" # c\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    let manifest = AppManifest::load(&*dir).unwrap();
    assert_eq!(manifest.name, "todo-api");
    assert_eq!(manifest.file, None);
    assert_eq!(manifest.paths, AppPaths::default());

    let layout = manifest.layout(&*dir);
    let found = |role: &str| layout.iter().find(|e| e.role == role).unwrap().clone();
    assert!(found("main").exists && found("main").kind == EntryKind::File);
    assert!(found("routes").exists && found("routes").kind == EntryKind::Dir);
    assert!(!found("lib").exists && !found("models").exists && !found("static").exists);
    assert_eq!(EntryKind::Dir.as_str(), "dir");
}

#[test]
fn load_reads_the_manifest_and_falls_back_to_the_directory_name() {
    let dir = TempPath::new("app_manifest_present");
    std::fs::create_dir_all(dir.join("public")).unwrap();
    std::fs::write(dir.join(FILE_NAME), "[paths]\nstatic = \"public\"\n").unwrap();
    // A template's placeholder is no name.
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"{{project-name}}\"\n").unwrap();
    let manifest = AppManifest::load(&*dir).unwrap();
    assert_eq!(manifest.file.as_deref(), Some(dir.join(FILE_NAME).as_path()));
    assert!(manifest.name.starts_with("rws_test_"), "{}", manifest.name);
    assert!(manifest.layout(&*dir).iter().any(|e| e.role == "static" && e.path == "public" && e.exists));
}

#[test]
fn load_reports_a_bad_manifest() {
    let dir = TempPath::new("app_manifest_bad");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(dir.join(FILE_NAME), "[paths]\nroutes = \"../x\"\n").unwrap();
    assert_eq!(AppManifest::load(&*dir).unwrap_err().line, 2);

    std::fs::write(dir.join(FILE_NAME), [0xff, 0xfe, 0x00]).unwrap();
    assert!(AppManifest::load(&*dir).unwrap_err().message.contains("UTF-8"));

    std::fs::remove_file(dir.join(FILE_NAME)).unwrap();
    std::fs::create_dir(dir.join(FILE_NAME)).unwrap();
    assert!(AppManifest::load(&*dir).unwrap_err().message.contains("not a file"));
}
