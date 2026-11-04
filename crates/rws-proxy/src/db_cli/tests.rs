use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn run_db(list: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args(list), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn cli(list: &[&str]) -> Cli {
    match parse(&args(list)) {
        Ok(Parsed::Run(cli)) => cli,
        Ok(Parsed::Help) => panic!("help for {list:?}"),
        Err(e) => panic!("{list:?}: {e}"),
    }
}

#[test]
fn requested_only_for_the_db_subcommand() {
    let argv = args(&["rws", "db", "status", "--json"]);
    assert_eq!(Some(&argv[2..]), requested(&argv));
    assert_eq!(None, requested(&args(&["rws", "gen", "model"])));
    assert_eq!(None, requested(&args(&["rws"])));
}

#[test]
fn parses_commands_and_options() {
    assert_eq!(Command::Migrate, cli(&["migrate"]).command);
    assert_eq!(Command::Status, cli(&["status"]).command);
    assert_eq!(Command::Seed, cli(&["seed"]).command);
    assert_eq!(Command::Rollback(1), cli(&["rollback"]).command);
    assert_eq!(Command::Rollback(3), cli(&["rollback", "3"]).command);
    let c = cli(&["migrate", "--database-url", "sqlite:x.db", "--dir=app", "--migrations", "db/m", "--seeds=db/s", "--json"]);
    assert_eq!(Some("sqlite:x.db".to_string()), c.database_url);
    assert_eq!((PathBuf::from("app"), Some(PathBuf::from("db/m")), Some(PathBuf::from("db/s")), true), (c.dir, c.migrations, c.seeds, c.json));
    assert!(matches!(parse(&args(&["--help"])), Ok(Parsed::Help)));
    assert!(matches!(parse(&args(&["help"])), Ok(Parsed::Help)));
}

#[test]
fn bad_arguments_are_usage_errors() {
    for (list, needle) in [
        (&[][..], "name a command"),
        (&["drop"][..], "unknown command `drop`"),
        (&["rollback", "0"][..], "a number from 1"),
        (&["rollback", "two"][..], "a number from 1"),
        (&["rollback", "1", "2"][..], "takes no `2`"),
        (&["status", "now"][..], "takes no `now`"),
        (&["migrate", "--force"][..], "unknown option --force"),
        (&["migrate", "--database-url"][..], "--database-url needs a value"),
    ] {
        let (code, out, err) = run_db(list);
        assert_eq!(EXIT_USAGE, code, "{list:?}");
        assert!(err.contains(needle) && err.contains("rws db --help"), "{list:?}: {err}");
        assert!(out.is_empty());
    }
    let (code, out, _) = run_db(&["drop", "--json"]);
    assert_eq!(EXIT_USAGE, code);
    assert!(out.starts_with(r#"{"error":"unknown command"#), "{out}");
    let (code, out, _) = run_db(&["--help"]);
    assert_eq!((EXIT_OK, true), (code, out.contains("rws db <command>")));
}

#[test]
fn seeds_live_beside_the_migrations() {
    assert_eq!(PathBuf::from("seeds"), seeds_beside(Path::new("migrations")));
    assert_eq!(PathBuf::from("seeds"), seeds_beside(Path::new("migrations/")));
    assert_eq!(PathBuf::from("db/seeds"), seeds_beside(Path::new("db/migrations")));
    assert_eq!(PathBuf::from("./seeds"), seeds_beside(Path::new("./migrations")));
    assert_eq!(PathBuf::from("/srv/app/seeds"), seeds_beside(Path::new("/srv/app/migrations")));
}

#[test]
fn directories_come_from_the_manifest() {
    let dir = crate::test_env::TempPath::new("db_cli_manifest");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(dir.join("rws.app.toml"), "[paths]\nmigrations = \"db/migrations\"\n").unwrap();
    let (migrations, seeds) = directories(&cli(&["status", "--dir", dir.as_str()])).unwrap();
    assert_eq!(dir.join("db/migrations").to_string_lossy(), migrations);
    assert_eq!(dir.join("db/seeds").to_string_lossy(), seeds);
    let (migrations, seeds) = directories(&cli(&["status", "--dir", dir.as_str(), "--migrations", "m", "--seeds", "s"])).unwrap();
    assert_eq!(("m", "s"), (migrations.as_str(), seeds.as_str()));

    std::fs::write(dir.join("rws.app.toml"), "[paths\n").unwrap();
    let (code, _, err) = run_db(&["status", "--dir", dir.as_str()]);
    assert_eq!(EXIT_FAILED, code, "{err}");
}

#[test]
fn renders_each_answer_for_a_person() {
    let status = r#"{"directory":"migrations","migrations":[{"version":"0001_a.sql","applied":true,"has_down":true},{"version":"0002_b.sql","applied":false,"has_down":false}]}"#;
    assert_eq!("applied  0001_a.sql\npending  0002_b.sql  (no .down.sql)\n", render(&Command::Status, status));
    assert_eq!("no migrations\n", render(&Command::Status, r#"{"directory":"m","migrations":[]}"#));
    assert_eq!("applied  0002_b.sql\n", render(&Command::Migrate, r#"{"applied":["0002_b.sql"],"directory":"m","migrations":[]}"#));
    assert!(render(&Command::Migrate, r#"{"applied":[],"directory":"m","migrations":[]}"#).starts_with("nothing to apply"));
    assert_eq!("rolled back  0002_b.sql\nrolled back  0001_a.sql\n", render(&Command::Rollback(2), r#"{"directory":"m","rolled_back":["0002_b.sql","0001_a.sql"]}"#));
    assert_eq!("seeded  01.sql\n", render(&Command::Seed, r#"{"directory":"seeds","seeded":["01.sql"]}"#));
}

#[cfg(not(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
#[test]
fn without_a_driver_it_says_how_to_get_one() {
    let (code, _, err) = run_db(&["status", "--migrations", "m"]);
    assert_eq!(EXIT_USAGE, code);
    assert!(err.contains("--features model-sqlite"), "{err}");
}

#[cfg(feature = "model-sqlite")]
#[test]
fn migrate_status_rollback_and_seed_against_sqlite() {
    let dir = crate::test_env::TempPath::new("db_cli_project");
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::create_dir_all(dir.join("seeds")).unwrap();
    std::fs::write(dir.join("migrations/0001_notes.sql"), "CREATE TABLE notes (id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL UNIQUE);").unwrap();
    std::fs::write(dir.join("migrations/0001_notes.down.sql"), "DROP TABLE notes;").unwrap();
    std::fs::write(dir.join("migrations/0002_tags.sql"), "CREATE TABLE tags (id INTEGER PRIMARY KEY);").unwrap();
    std::fs::write(dir.join("seeds/01_notes.sql"), "INSERT OR IGNORE INTO notes (text) VALUES ('hello'); INSERT OR IGNORE INTO notes (text) VALUES ('world');").unwrap();
    let url = format!("sqlite:{}?mode=rwc", dir.join("app.db").display());
    let db = |list: &[&str]| {
        let mut full: Vec<&str> = list.to_vec();
        full.extend(["--dir", dir.as_str(), "--database-url", url.as_str()]);
        run_db(&full)
    };

    let (code, out, err) = db(&["status"]);
    assert_eq!(EXIT_OK, code, "{err}");
    assert_eq!("pending  0001_notes.sql\npending  0002_tags.sql  (no .down.sql)\n", out);

    let (code, out, err) = db(&["migrate", "--json"]);
    assert_eq!(EXIT_OK, code, "{err}");
    assert!(out.starts_with(r#"{"applied":["0001_notes.sql","0002_tags.sql"],"directory":"#), "{out}");
    let (_, out, _) = db(&["migrate"]);
    assert!(out.starts_with("nothing to apply"), "{out}");

    let (code, out, err) = db(&["seed"]);
    assert_eq!((EXIT_OK, "seeded  01_notes.sql\n"), (code, out.as_str()), "{err}");
    assert_eq!(EXIT_OK, db(&["seed"]).0, "INSERT OR IGNORE: runs again");

    // 0002 has no down file: rollback stops there with an error.
    let (code, out, err) = db(&["rollback", "--json"]);
    assert_eq!(EXIT_FAILED, code);
    assert!(out.contains("0002_tags.down.sql"), "{out} {err}");
    std::fs::write(dir.join("migrations/0002_tags.down.sql"), "DROP TABLE tags;").unwrap();
    let (code, out, err) = db(&["rollback", "5"]);
    assert_eq!(EXIT_OK, code, "{err}");
    assert_eq!("rolled back  0002_tags.sql\nrolled back  0001_notes.sql\n", out);

    let (code, out, _) = db(&["status", "--json"]);
    assert_eq!(EXIT_OK, code);
    assert!(out.contains(r#""version":"0001_notes.sql","applied":false"#), "{out}");

    // The status JSON is the dev MCP server's `migrations` answer.
    assert!(out.starts_with(r#"{"directory":"#) && out.contains(r#""migrations":["#), "{out}");

    let (code, _, err) = run_db(&["status", "--dir", dir.as_str(), "--database-url", "oracle://x"]);
    assert_eq!(EXIT_FAILED, code);
    assert!(err.contains("expected sqlite:"), "{err}");
    let (code, _, err) = db(&["seed", "--seeds", "/no/such/seeds"]);
    assert_eq!(EXIT_FAILED, code);
    assert!(err.contains("seeds directory"), "{err}");
}

/// `rws db` on a server's database (`RWS_TEST_POSTGRES_URL` /
/// `RWS_TEST_MYSQL_URL`; skipped without, failed when
/// `RWS_TEST_REQUIRE_BACKENDS` names the backend). The database is shared,
/// so the migration's version sorts last (`9999_…`) and is rolled back.
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn against_server(var: &str, name: &str) {
    let Some(url) = std::env::var(var).ok().filter(|u| !u.trim().is_empty()) else {
        let required = std::env::var("RWS_TEST_REQUIRE_BACKENDS").unwrap_or_default();
        assert!(!required.split(',').any(|r| r.trim() == name), "RWS_TEST_REQUIRE_BACKENDS lists {name}, but {var} is not set");
        eprintln!("skipped {name}: {var} is not set");
        return;
    };
    let tag = format!("{name}_{}", std::process::id());
    let dir = crate::test_env::TempPath::new(&format!("db_cli_{tag}"));
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::create_dir_all(dir.join("seeds")).unwrap();
    let version = format!("9999_rws_db_cli_{tag}.sql");
    std::fs::write(dir.join("migrations").join(&version), format!("CREATE TABLE rws_db_cli_{tag} (id INTEGER PRIMARY KEY, note VARCHAR(64))")).unwrap();
    std::fs::write(dir.join("migrations").join(version.replace(".sql", ".down.sql")), format!("DROP TABLE rws_db_cli_{tag}")).unwrap();
    std::fs::write(dir.join("seeds/01.sql"), format!("INSERT INTO rws_db_cli_{tag} (id, note) VALUES (1, 'seeded')")).unwrap();
    let db = |list: &[&str]| {
        let mut full: Vec<&str> = list.to_vec();
        full.extend(["--dir", dir.as_str(), "--database-url", url.as_str()]);
        run_db(&full)
    };
    let (code, out, err) = db(&["migrate"]);
    assert_eq!(EXIT_OK, code, "[{name}] {err}");
    assert_eq!(format!("applied  {version}\n"), out);
    let (code, out, err) = db(&["seed"]);
    assert_eq!((EXIT_OK, "seeded  01.sql\n"), (code, out.as_str()), "[{name}] {err}");
    let (code, out, _) = db(&["status"]);
    assert_eq!((EXIT_OK, format!("applied  {version}\n")), (code, out), "[{name}]");
    let (code, out, err) = db(&["rollback"]);
    assert_eq!((EXIT_OK, format!("rolled back  {version}\n")), (code, out), "[{name}] {err}");
    let (_, out, _) = db(&["status"]);
    assert_eq!(format!("pending  {version}\n"), out, "[{name}]");
}

#[cfg(feature = "model-postgres")]
#[test]
fn migrate_seed_and_rollback_against_postgres() {
    against_server("RWS_TEST_POSTGRES_URL", "postgres");
}

#[cfg(feature = "model-mysql")]
#[test]
fn migrate_seed_and_rollback_against_mysql() {
    against_server("RWS_TEST_MYSQL_URL", "mysql");
}
