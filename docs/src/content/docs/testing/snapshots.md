---
title: OpenAPI snapshots
description: Compare the generated OpenAPI document with a committed file; RWS_UPDATE_SNAPSHOTS=1 rewrites it.
---

`assert_openapi_snapshot` (since 18.29.0) compares an app's OpenAPI document with a file committed beside the tests, so a change to the API shows up in review as a change to that file:

```rust
use rust_web_server::app::App;
use rust_web_server::openapi::OpenApiConfig;
use rust_web_server::test_client::{assert_openapi_snapshot, TestClient};

#[test]
fn api_matches_the_snapshot() {
    let app = my_app::app().openapi(OpenApiConfig::new("Shop", "1.0.0"));
    assert_openapi_snapshot(&TestClient::new(app), "tests/snapshots/openapi.json");
}
```

The first argument is anything the document can be read from: a `TestClient` whose app serves `GET /openapi.json` (`AppWithState::openapi`), a `TestResponse` of it, or the JSON text itself (`app.openapi_spec(&config)`). A document that isn't JSON or has no `openapi` field fails — a `404` page is no snapshot.

Both sides are normalized before comparing — parsed, object keys sorted, printed with two-space indentation — so key order and whitespace don't count. A mismatch fails with a line diff (`-` the snapshot, `+` now, three lines of context):

```text
the document differs from the snapshot tests/snapshots/openapi.json (- snapshot, + now):
  …
            "200": {
-             "description": "OK"
+             "description": "Found"
            }
  …
If the change is intended, run the test with RWS_UPDATE_SNAPSHOTS=1 and commit the file.
```

## Updating

```bash
RWS_UPDATE_SNAPSHOTS=1 cargo test     # write the current documents
git diff tests/snapshots/             # review what changed, then commit
```

With `RWS_UPDATE_SNAPSHOTS` set (to anything but empty, `0` or `false`) the assertion writes the normalized document — creating the file and its directory — and passes. Without it a missing file fails with that hint, so CI never passes by writing its own snapshot. `RWS_UPDATE_SNAPSHOTS` is read by the test helpers only; it isn't a server setting.

Relative paths are relative to the working directory, which under `cargo test` is the package root.

`assert_json_snapshot(json, path)` does the same for any JSON document — a JSON Schema, an MCP `tools/list` result, a response body.
