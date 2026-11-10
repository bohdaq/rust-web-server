use super::*;
use crate::app::App;
use crate::test_client::{TestClient, TestResponse};

fn ddl(backend: Backend) -> Vec<String> {
    let (pk, boolean, ts) = match backend {
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => ("BIGSERIAL PRIMARY KEY", "BOOLEAN", "TIMESTAMPTZ"),
        #[cfg(feature = "model-mysql")]
        Backend::MySql => ("BIGINT AUTO_INCREMENT PRIMARY KEY", "BOOLEAN", "DATETIME(6)"),
        #[allow(unreachable_patterns)]
        // SQLite keeps the declared type name: BOOLEAN shows as a checkbox
        // (a column declared INTEGER is a number field).
        _ => ("INTEGER PRIMARY KEY AUTOINCREMENT", "BOOLEAN", "TEXT"),
    };
    vec![
        "DROP TABLE IF EXISTS adm_posts".into(),
        "DROP TABLE IF EXISTS adm_links".into(),
        format!("CREATE TABLE adm_posts (id {pk}, title VARCHAR(200) NOT NULL UNIQUE, body TEXT, views BIGINT NOT NULL, published {boolean} NOT NULL, created_at {ts} NOT NULL, updated_at {ts} NOT NULL)"),
        "CREATE TABLE adm_links (a BIGINT NOT NULL, b BIGINT NOT NULL, PRIMARY KEY (a, b))".into(),
    ]
}

fn admin_on(pool: DbPool) -> TestClient<impl Application> {
    let admin = ModelAdmin::new(pool).title("Notes admin").table("adm_posts").table("adm_links").table("bad name; drop").allow(|r| r.header("x-admin").is_some());
    TestClient::new(App::with_state(()).get("/", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "app")).wrap(admin))
}

fn form<A: Application>(client: &TestClient<A>, path: &str, body: &str) -> TestResponse {
    client.post(path).header("X-Admin", "1").header("Content-Type", "application/x-www-form-urlencoded").body_text(body).send()
}

fn page<A: Application>(client: &TestClient<A>, path: &str) -> TestResponse {
    client.get(path).header("X-Admin", "1").send()
}

fn flash_of(response: &TestResponse) -> String {
    response.headers().iter().find(|h| h.value.starts_with("rws_flash=")).map(|h| h.value.to_string()).unwrap_or_default()
}

/// The whole flow on one database.
fn exercise(pool: DbPool, name: &str) {
    let backend = pool.backend();
    rws_server::async_bridge::block_on_shared({
        let pool = pool.clone();
        async move {
            for sql in ddl(backend) {
                pool.execute(&sql, &[]).await?;
            }
            Ok::<_, crate::model::DbError>(())
        }
    })
    .unwrap();
    let client = admin_on(pool);

    // Nobody without the allow check; other paths untouched.
    assert_eq!(403, client.get("/admin").send().status(), "[{name}]");
    assert_eq!(403, client.post("/admin/adm_posts").send().status());
    assert_eq!("app", client.get("/").send().body_text());
    assert_eq!(404, client.get("/administrator").send().status(), "only the prefix as a whole path segment: the app answers");

    let index = page(&client, "/admin");
    assert_eq!(200, index.status());
    assert!(index.body_text().contains(">adm_posts</a></td><td>0</td>"), "[{name}] {}", index.body_text());
    assert!(!index.body_text().contains("bad name"), "invalid table names aren't registered");
    assert_eq!(404, page(&client, "/admin/secrets").status(), "unregistered tables aren't served");

    // The new form leaves out the auto key; booleans are checkboxes.
    let new = page(&client, "/admin/adm_posts/new").body_text().to_string();
    assert!(new.contains(r#"name="title""#) && new.contains(r#"type="checkbox" name="published""#) && !new.contains(r#"name="id""#), "[{name}] {new}");

    // Create: converted by type, timestamps filled in.
    let created = form(&client, "/admin/adm_posts", "title=Hello+%3Cworld%3E&body=&views=7&published=on");
    assert_eq!((303, Some("/admin/adm_posts")), (created.status(), created.header("location")), "[{name}] {}", flash_of(&created));
    assert!(flash_of(&created).contains("notice=Created"));
    let list = page(&client, "/admin/adm_posts").body_text().to_string();
    assert!(list.contains("Hello &lt;world&gt;") && list.contains("1 rows"), "[{name}] escaped: {list}");
    let id = list.split("<td><a href=\"/admin/adm_posts/").nth(1).unwrap().split('"').next().unwrap().to_string();
    let show = page(&client, &format!("/admin/adm_posts/{id}")).body_text().to_string();
    assert!(show.contains("<th>views</th><td>7</td>"), "[{name}] {show}");
    assert!(show.contains("<th>body</th><td></td>"), "[{name}] an empty optional field is NULL: {show}");

    // Bad input and refusals come back as alerts on the form.
    let bad = form(&client, "/admin/adm_posts", "title=Other&views=lots");
    assert_eq!(Some("/admin/adm_posts/new"), bad.header("location"));
    assert!(flash_of(&bad).contains("whole%20number"), "{}", flash_of(&bad));
    let duplicate = form(&client, "/admin/adm_posts", "title=Hello+%3Cworld%3E&views=1");
    assert!(flash_of(&duplicate).contains("refused"), "[{name}] unique index: {}", flash_of(&duplicate));

    // Edit and update.
    let edit = page(&client, &format!("/admin/adm_posts/{id}/edit")).body_text().to_string();
    assert!(edit.contains(r#"value="7""#) && edit.contains(" checked"), "[{name}] {edit}");
    let saved = form(&client, &format!("/admin/adm_posts/{id}"), "title=Renamed&views=8");
    assert_eq!(Some(format!("/admin/adm_posts/{id}").as_str()), saved.header("location"), "[{name}] {}", flash_of(&saved));
    let show = page(&client, &format!("/admin/adm_posts/{id}")).body_text().to_string();
    assert!(show.contains("Renamed") && show.contains("<th>views</th><td>8</td>"), "[{name}] {show}");
    assert!(!show.contains("<th>created_at</th><td></td>"), "[{name}] fields the form didn't send stay: {show}");
    assert!(show.contains("<th>published</th><td>false</td>") || show.contains("<th>published</th><td>0</td>"), "[{name}] an unchecked box is false: {show}");
    assert_eq!(404, form(&client, "/admin/adm_posts/999999", "title=x&views=1").status());

    // Search and paging.
    for n in 0..30 {
        form(&client, "/admin/adm_posts", &format!("title=bulk+{n}&views={n}"));
    }
    let first = page(&client, "/admin/adm_posts").body_text().to_string();
    assert!(first.contains("31 rows · page 1 of 2") && first.contains(">older</a>"), "[{name}] {first}");
    let searched = page(&client, "/admin/adm_posts?q=RENAMED").body_text().to_string();
    assert!(searched.contains("1 rows") && searched.contains("Renamed"), "[{name}] case-insensitive: {searched}");
    assert!(page(&client, "/admin/adm_posts?page=2").body_text().contains("page 2 of 2"));

    // Delete.
    let deleted = form(&client, &format!("/admin/adm_posts/{id}/delete"), "");
    assert!(flash_of(&deleted).contains("Deleted"));
    assert_eq!(404, page(&client, &format!("/admin/adm_posts/{id}")).status());
    assert_eq!(404, form(&client, &format!("/admin/adm_posts/{id}/delete"), "").status());

    // A composite key: listed, not edited.
    let links = page(&client, "/admin/adm_links").body_text().to_string();
    assert!(links.contains("can't be edited"), "[{name}] {links}");
    assert_eq!(404, page(&client, "/admin/adm_links/new").status());
    assert_eq!(404, page(&client, "/admin/adm_posts/1/2/3").status());
}


#[cfg(feature = "model-sqlite")]
#[test]
fn the_admin_on_sqlite() {
    let pool = rws_server::async_bridge::block_on_shared(async { DbPool::memory().await }).unwrap();
    exercise(pool, "sqlite");
}

/// The same on PostgreSQL / MySQL when `RWS_TEST_POSTGRES_URL` /
/// `RWS_TEST_MYSQL_URL` name a server (the only test using `adm_*` there).
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
#[test]
fn the_admin_on_real_servers() {
    for (name, var) in [("postgres", "RWS_TEST_POSTGRES_URL"), ("mysql", "RWS_TEST_MYSQL_URL")] {
        if (name == "postgres" && !cfg!(feature = "model-postgres")) || (name == "mysql" && !cfg!(feature = "model-mysql")) {
            continue;
        }
        let Ok(url) = std::env::var(var) else {
            assert!(!std::env::var("RWS_TEST_REQUIRE_BACKENDS").unwrap_or_default().contains(name), "{var} isn't set");
            continue;
        };
        let pool = rws_server::async_bridge::block_on_shared(async move { DbPool::connect(&url).await }).unwrap();
        exercise(pool.clone(), name);
        rws_server::async_bridge::block_on_shared(async move {
            pool.execute("DROP TABLE adm_posts", &[]).await?;
            pool.execute("DROP TABLE adm_links", &[]).await?;
            Ok::<_, crate::model::DbError>(())
        })
        .unwrap();
    }
}

#[test]
fn values_are_read_by_column_type() {
    let p = |data_type: &str, nullable: bool, value: Option<&str>| parse_as("c", data_type, nullable, value);
    assert_eq!(Ok(Value::Int(5)), p("BIGINT", false, Some(" 5 ")));
    assert!(p("integer", false, Some("x")).is_err());
    assert_eq!(Ok(Value::Float(1.5)), p("double precision", false, Some("1.5")));
    assert_eq!(Ok(Value::Bool(true)), p("boolean", false, Some("on")));
    assert_eq!(Ok(Value::Bool(false)), p("tinyint(1)", false, None));
    assert_eq!(Ok(Value::Null), p("text", true, Some("")));
    assert_eq!(Ok(Value::Text(String::new())), p("TEXT", false, None));
    assert!(p("bigint", false, Some("")).is_err(), "required");
    assert_eq!(Kind::Timestamp, kind_of("timestamp with time zone"));
    assert_eq!(Kind::Other, kind_of("uuid"));
    assert_eq!(Kind::Text, kind_of("character varying"));
    #[cfg(feature = "model-sqlite")]
    assert_eq!("\"ab\"", quote(Backend::Sqlite, "a\"b"), "quotes can't be escaped out of");
}
