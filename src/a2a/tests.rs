use super::*;
use crate::service_discovery::json_lite;
use crate::test_client::TestClient;

fn agent() -> A2aServer {
    A2aServer::new(AgentCard::new("Echo agent", "Repeats what you say", "1.2.0").skill(Skill::new("echo", "Echo", "Repeats the message").tag("demo").example("hello")))
        .on_message(|ctx, message| match message.text().as_str() {
            "" => A2aReply::Message(Message::agent_text("Say something.")),
            "ask" => A2aReply::Task(ctx.input_required("Which one?")),
            "panic" => panic!("handler bug"),
            text if ctx.task.is_some() => A2aReply::Task(ctx.complete(vec![Artifact::text("answer", format!("you chose {text}"))])),
            "work" => A2aReply::Task(ctx.task(TaskState::Working, None, Vec::new())),
            text => A2aReply::Task(ctx.complete(vec![Artifact::text("echo", text)])),
        })
}

fn call(client: &TestClient<A2aServer>, method: &str, params: &str) -> json_lite::JsonValue {
    let body = format!(r#"{{"jsonrpc":"2.0","id":7,"method":"{method}","params":{params}}}"#);
    let response = client.post("/a2a").header("A2A-Version", "1.0").body_json(&body).send();
    assert_eq!(200, response.status());
    json_lite::parse(response.body_text()).unwrap()
}

fn send(client: &TestClient<A2aServer>, text: &str, extra: &str) -> json_lite::JsonValue {
    let message = format!(r#"{{"message":{{"messageId":"{}","role":"ROLE_USER","parts":[{{"text":"{text}"}}]{extra}}}}}"#, new_id());
    call(client, "SendMessage", &message)
}

fn s<'a>(v: &'a json_lite::JsonValue, path: &[&str]) -> &'a str {
    let mut cur = v;
    for key in path {
        cur = cur.get(key).unwrap_or_else(|| panic!("no {key} in {}", model::to_json(v)));
    }
    cur.as_str().unwrap_or_else(|| panic!("{path:?} isn't a string in {}", model::to_json(v)))
}

fn error_code(v: &json_lite::JsonValue) -> i64 {
    v.get("error").and_then(|e| e.get("code")).and_then(|c| c.as_f64()).unwrap_or_else(|| panic!("no error in {}", model::to_json(v))) as i64
}

#[test]
fn the_agent_card() {
    let client = TestClient::new(agent());
    let response = client.get(AGENT_CARD_PATH).header("Host", "agents.example.com").send();
    assert_eq!(200, response.status());
    let card = json_lite::parse(response.body_text()).unwrap();
    assert_eq!("Echo agent", s(&card, &["name"]));
    assert_eq!("1.2.0", s(&card, &["version"]));
    let interface = &card.get("supportedInterfaces").unwrap().as_array().unwrap()[0];
    assert_eq!(("http://agents.example.com/a2a", "JSONRPC", "1.0"), (s(interface, &["url"]), s(interface, &["protocolBinding"]), s(interface, &["protocolVersion"])));
    assert_eq!(Some(&json_lite::JsonValue::Bool(false)), card.get("capabilities").unwrap().get("streaming"));
    let skill = &card.get("skills").unwrap().as_array().unwrap()[0];
    assert_eq!(("echo", "demo"), (s(skill, &["id"]), skill.get("tags").unwrap().as_array().unwrap()[0].as_str().unwrap()));
    assert_eq!("text/plain", card.get("defaultInputModes").unwrap().as_array().unwrap()[0].as_str().unwrap());

    let fixed = TestClient::new(A2aServer::new(AgentCard::new("a", "b", "1").url("https://public.example/agent")));
    let card = json_lite::parse(fixed.get(AGENT_CARD_PATH).send().body_text()).unwrap();
    assert_eq!("https://public.example/agent", s(&card.get("supportedInterfaces").unwrap().as_array().unwrap()[0], &["url"]));
}

#[test]
fn messages_get_a_message_or_a_task() {
    let client = TestClient::new(agent());
    let reply = send(&client, "", "");
    let message = reply.get("result").unwrap().get("message").unwrap();
    assert_eq!(("ROLE_AGENT", "Say something."), (s(message, &["role"]), s(&message.get("parts").unwrap().as_array().unwrap()[0], &["text"])));
    assert!(!s(message, &["contextId"]).is_empty());

    let reply = send(&client, "hello", r#","contextId":"ctx-1""#);
    let task = reply.get("result").unwrap().get("task").unwrap();
    assert_eq!("TASK_STATE_COMPLETED", s(task, &["status", "state"]));
    assert_eq!("ctx-1", s(task, &["contextId"]));
    assert!(s(task, &["status", "timestamp"]).ends_with('Z'));
    let artifact = &task.get("artifacts").unwrap().as_array().unwrap()[0];
    assert_eq!("hello", s(&artifact.get("parts").unwrap().as_array().unwrap()[0], &["text"]));
    let id = s(task, &["id"]).to_string();

    let got = call(&client, "GetTask", &format!(r#"{{"id":"{id}"}}"#));
    let got = got.get("result").unwrap();
    assert_eq!(id, s(got, &["id"]));
    assert_eq!(1, got.get("history").unwrap().as_array().unwrap().len());
    let none = call(&client, "GetTask", &format!(r#"{{"id":"{id}","historyLength":0}}"#));
    assert!(none.get("result").unwrap().get("history").is_none(), "historyLength 0: no messages");
    assert_eq!(-32001, error_code(&call(&client, "GetTask", r#"{"id":"nope"}"#)));
}

#[test]
fn a_task_waiting_for_input_continues_and_then_ends() {
    let client = TestClient::new(agent());
    let reply = send(&client, "ask", "");
    let task = reply.get("result").unwrap().get("task").unwrap();
    assert_eq!("TASK_STATE_INPUT_REQUIRED", s(task, &["status", "state"]));
    assert_eq!("Which one?", s(&task.get("status").unwrap().get("message").unwrap().get("parts").unwrap().as_array().unwrap()[0], &["text"]));
    let id = s(task, &["id"]).to_string();

    let reply = send(&client, "blue", &format!(r#","taskId":"{id}""#));
    let task = reply.get("result").unwrap().get("task").unwrap();
    assert_eq!((id.as_str(), "TASK_STATE_COMPLETED"), (s(task, &["id"]), s(task, &["status", "state"])));
    assert_eq!(3, task.get("history").unwrap().as_array().unwrap().len(), "ask, the question, blue");
    assert_eq!(-32004, error_code(&send(&client, "again", &format!(r#","taskId":"{id}""#))), "a finished task takes no more messages");
    assert_eq!(-32001, error_code(&send(&client, "x", r#","taskId":"unknown""#)));
}

#[test]
fn list_and_cancel_tasks() {
    let client = TestClient::new(agent());
    for text in ["a", "b", "c"] {
        send(&client, text, r#","contextId":"ctx-list""#);
    }
    let working = send(&client, "work", "");
    let working_id = s(working.get("result").unwrap().get("task").unwrap(), &["id"]).to_string();

    let page = call(&client, "ListTasks", r#"{"contextId":"ctx-list","pageSize":2}"#);
    let page = page.get("result").unwrap();
    assert_eq!(2, page.get("tasks").unwrap().as_array().unwrap().len());
    assert_eq!(Some(3.0), page.get("totalSize").unwrap().as_f64());
    let next = s(page, &["nextPageToken"]).to_string();
    let rest = call(&client, "ListTasks", &format!(r#"{{"contextId":"ctx-list","pageSize":2,"pageToken":"{next}"}}"#));
    assert_eq!(1, rest.get("result").unwrap().get("tasks").unwrap().as_array().unwrap().len());
    assert_eq!("", s(rest.get("result").unwrap(), &["nextPageToken"]));
    let working_only = call(&client, "ListTasks", r#"{"status":"TASK_STATE_WORKING"}"#);
    assert_eq!(1, working_only.get("result").unwrap().get("tasks").unwrap().as_array().unwrap().len());
    assert_eq!(-32602, error_code(&call(&client, "ListTasks", r#"{"pageSize":1000}"#)));

    let canceled = call(&client, "CancelTask", &format!(r#"{{"id":"{working_id}"}}"#));
    assert_eq!("TASK_STATE_CANCELED", s(canceled.get("result").unwrap(), &["status", "state"]));
    assert_eq!(-32002, error_code(&call(&client, "CancelTask", &format!(r#"{{"id":"{working_id}"}}"#))));
}

#[test]
fn errors_and_versions() {
    let client = TestClient::new(agent());
    let raw = |body: &str, version: Option<&str>| {
        let mut request = client.post("/a2a").body_json(body);
        if let Some(v) = version {
            request = request.header("A2A-Version", v);
        }
        json_lite::parse(request.send().body_text()).unwrap()
    };
    assert_eq!(-32700, error_code(&raw("{not json", Some("1.0"))));
    assert_eq!(-32600, error_code(&raw(r#"{"id":1,"method":"GetTask"}"#, Some("1.0"))));
    let missing = raw(r#"{"jsonrpc":"2.0","id":"x","method":"GetTask","params":{}}"#, None);
    assert_eq!(-32009, error_code(&missing), "no A2A-Version means 0.3");
    assert_eq!("x", s(&missing, &["id"]), "the id is echoed");
    assert_eq!("VERSION_NOT_SUPPORTED", s(&missing.get("error").unwrap().get("data").unwrap().as_array().unwrap()[0], &["reason"]));
    assert!(raw(r#"{"jsonrpc":"2.0","id":1,"method":"GetTask","params":{"id":"n"}}"#, Some("1.0.3")).get("error").is_some_and(|e| e.get("code").unwrap().as_f64() == Some(-32001.0)), "patch versions ignored");
    let query = client.post("/a2a?A2A-Version=1.0").body_json(r#"{"jsonrpc":"2.0","id":1,"method":"Nope"}"#).send();
    assert_eq!(-32601, error_code(&json_lite::parse(query.body_text()).unwrap()), "version from the query, then unknown method");

    assert_eq!(-32602, error_code(&call(&client, "SendMessage", r#"{"message":{"messageId":"m","role":"ROLE_USER","parts":[{"text":"a","url":"b"}]}}"#)));
    assert_eq!(-32602, error_code(&call(&client, "SendMessage", r#"{"message":{"role":"ROLE_USER","parts":[{"text":"a"}]}}"#)));
    assert_eq!(-32602, error_code(&call(&client, "SendMessage", "{}")));
    assert_eq!(-32004, error_code(&call(&client, "SendStreamingMessage", "{}")));
    assert_eq!(-32003, error_code(&call(&client, "CreateTaskPushNotificationConfig", "{}")));
    assert_eq!(-32004, error_code(&call(&client, "GetExtendedAgentCard", "{}")));

    let failed = send(&client, "panic", "");
    assert_eq!("TASK_STATE_FAILED", s(failed.get("result").unwrap().get("task").unwrap(), &["status", "state"]), "a panicking handler fails its task");

    let get = client.get("/a2a").send();
    assert_eq!((405, Some("POST")), (get.status(), get.header("Allow")));
    assert_eq!(404, client.get("/other").send().status());
}

#[test]
fn parts_round_trip_and_requests_fall_through() {
    let data = r#"{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"raw":"aGk=","mediaType":"text/plain","filename":"hi.txt"},{"url":"https://x/y.png","mediaType":"image/png"},{"data":{"n":[1,2.5,true,null],"s":"q\"uote"}}],"metadata":{"k":"v"},"referenceTaskIds":["t0"]}}"#;
    let params = json_lite::parse(data).unwrap();
    let message = Message::from_json(params.get("message").unwrap()).unwrap();
    assert_eq!(PartContent::Raw(b"hi".to_vec()), message.parts[0].content);
    assert_eq!(Some("hi.txt"), message.parts[0].filename.as_deref());
    assert_eq!(PartContent::Data(r#"{"n":[1,2.5,true,null],"s":"q\"uote"}"#.to_string()), message.parts[2].content);
    let again = Message::from_json(&json_lite::parse(&message.to_json()).unwrap()).unwrap();
    assert_eq!(message, again);

    use crate::core::New;
    let app = agent().wrap(crate::app::App::new());
    let client = TestClient::new(app);
    assert_eq!(200, client.get("/healthz").send().status());
    assert!(model::timestamp().len() == 24 && model::timestamp().ends_with('Z'));
    let id = new_id();
    assert_eq!((36, Some('4')), (id.len(), id.chars().nth(14)));
}
