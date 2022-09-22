use crate::app::App;
use crate::{override_environment_variables_from_config, Request};
use crate::request::METHOD;
use crate::http::VERSION;
use crate::response::STATUS_CODE_REASON_PHRASE;

#[test]
fn post() {
    override_environment_variables_from_config(Some("/src/test/rws.config.toml"));

    let request = Request {
        method: METHOD.post.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![]
    };

    let (response, _request) = App::handle_request(request);
    assert_eq!(response.status_code, STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
}