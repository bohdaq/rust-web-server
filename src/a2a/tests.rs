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
            "slow" => {
                let task = ctx.start();
                let worker = task.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    worker.working(Some("halfway"));
                    worker.artifact(Artifact::text("result", "42"));
                    worker.complete();
                });
                A2aReply::Running(task)
            }
            "forever" => {
                let task = ctx.start();
                let worker = task.clone();
                std::thread::spawn(move || {
                    while !worker.is_canceled() {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                });
                A2aReply::Running(task)
            }
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
    assert_eq!(Some(&json_lite::JsonValue::Bool(true)), card.get("capabilities").unwrap().get("streaming"));
    assert!(card.get("securitySchemes").is_none(), "no auth configured");
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
    assert_eq!(-32602, error_code(&call(&client, "SendStreamingMessage", "{}")), "streams need a message too");
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

fn stream_events(response: crate::response::Response) -> Vec<json_lite::JsonValue> {
    use std::io::Read;
    let mut text = String::new();
    response.stream_pipe.expect("an SSE stream").read_to_string(&mut text).unwrap();
    text.lines().filter_map(|l| l.strip_prefix("data: ")).map(|d| json_lite::parse(d).unwrap()).collect()
}

fn rpc_request(method: &str, params: &str, token: Option<&str>) -> crate::request::Request {
    let body = format!(r#"{{"jsonrpc":"2.0","id":"s1","method":"{method}","params":{params}}}"#);
    let mut headers = vec![crate::header::Header::new("A2A-Version", "1.0".to_string())];
    if let Some(t) = token {
        headers.push(crate::header::Header::new("Authorization", format!("Bearer {t}")));
    }
    crate::request::Request { method: "POST".into(), request_uri: "/a2a".into(), http_version: "HTTP/1.1".into(), headers, body: body.into_bytes().into() }
}

fn message_params(text: &str, extra: &str) -> String {
    format!(r#"{{"message":{{"messageId":"{}","role":"ROLE_USER","parts":[{{"text":"{text}"}}]}}{extra}}}"#, new_id())
}

#[test]
fn send_message_waits_for_a_running_task_unless_told_not_to() {
    let client = TestClient::new(agent());
    let started = std::time::Instant::now();
    let done = call(&client, "SendMessage", &message_params("slow", ""));
    let task = done.get("result").unwrap().get("task").unwrap();
    assert_eq!("TASK_STATE_COMPLETED", s(task, &["status", "state"]));
    assert!(started.elapsed() >= std::time::Duration::from_millis(50), "it waited for the worker");
    assert_eq!("42", s(&task.get("artifacts").unwrap().as_array().unwrap()[0].get("parts").unwrap().as_array().unwrap()[0], &["text"]));
    assert_eq!("halfway", s(&task.get("history").unwrap().as_array().unwrap()[1].get("parts").unwrap().as_array().unwrap()[0], &["text"]));

    let quick = call(&client, "SendMessage", &message_params("slow", r#","configuration":{"returnImmediately":true}"#));
    assert_eq!("TASK_STATE_WORKING", s(quick.get("result").unwrap().get("task").unwrap(), &["status", "state"]));

    let impatient = TestClient::new(agent().blocking_timeout(std::time::Duration::from_millis(50)));
    let reply = call(&impatient, "SendMessage", &message_params("forever", ""));
    let task = reply.get("result").unwrap().get("task").unwrap();
    assert_eq!("TASK_STATE_WORKING", s(task, &["status", "state"]), "past blocking_timeout: the current state");
    let id = s(task, &["id"]).to_string();
    let canceled = call(&impatient, "CancelTask", &format!(r#"{{"id":"{id}"}}"#));
    assert_eq!("TASK_STATE_CANCELED", s(canceled.get("result").unwrap(), &["status", "state"]));
}

#[test]
fn streams_send_the_task_then_its_updates() {
    use crate::application::Application;
    let agent = agent();
    let conn = crate::server::ConnectionInfo::default();

    let events = stream_events(agent.execute(&rpc_request("SendStreamingMessage", &message_params("slow", ""), None), &conn).unwrap());
    let kinds: Vec<&str> = events.iter().map(|e| match e.get("result").unwrap() {
        json_lite::JsonValue::Object(pairs) => pairs[0].0.as_str(),
        _ => "?",
    }).collect();
    assert_eq!(vec!["task", "statusUpdate", "artifactUpdate", "statusUpdate"], kinds);
    assert_eq!("s1", s(&events[0], &["id"]));
    assert_eq!("TASK_STATE_WORKING", s(&events[0], &["result", "task", "status", "state"]));
    assert_eq!("TASK_STATE_COMPLETED", s(&events[3], &["result", "statusUpdate", "status", "state"]));
    assert_eq!("result", s(&events[2], &["result", "artifactUpdate", "artifact", "name"]));

    let one = stream_events(agent.execute(&rpc_request("SendStreamingMessage", &message_params("", ""), None), &conn).unwrap());
    assert_eq!(1, one.len());
    assert_eq!("Say something.", s(&one[0].get("result").unwrap().get("message").unwrap().get("parts").unwrap().as_array().unwrap()[0], &["text"]));
    let asked = stream_events(agent.execute(&rpc_request("SendStreamingMessage", &message_params("ask", ""), None), &conn).unwrap());
    assert_eq!(1, asked.len(), "a task waiting for input ends the stream");

    // SubscribeToTask: the current task first, then updates.
    let running = agent.execute(&rpc_request("SendMessage", &message_params("slow", r#","configuration":{"returnImmediately":true}"#), None), &conn).unwrap();
    let running = json_lite::parse(std::str::from_utf8(&running.content_range_list[0].body).unwrap()).unwrap();
    let id = s(&running, &["result", "task", "id"]).to_string();
    let events = stream_events(agent.execute(&rpc_request("SubscribeToTask", &format!(r#"{{"id":"{id}"}}"#), None), &conn).unwrap());
    assert_eq!("TASK_STATE_WORKING", s(&events[0], &["result", "task", "status", "state"]));
    assert_eq!("TASK_STATE_COMPLETED", s(events.last().unwrap(), &["result", "statusUpdate", "status", "state"]));
    let finished = agent.execute(&rpc_request("SubscribeToTask", &format!(r#"{{"id":"{id}"}}"#), None), &conn).unwrap();
    let finished = json_lite::parse(std::str::from_utf8(&finished.content_range_list[0].body).unwrap()).unwrap();
    assert_eq!(-32004, error_code(&finished), "a finished task can't be subscribed to");
}

#[test]
fn bearer_tokens_and_per_caller_tasks() {
    use crate::application::Application;
    let agent = agent().require_bearer_tokens([("t-alice", "alice"), ("t-bob", "bob")]);
    let conn = crate::server::ConnectionInfo::default();
    let body = |r: crate::response::Response| (r.status_code, String::from_utf8(r.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default()).unwrap());

    let (status, _) = body(agent.execute(&rpc_request("GetTask", r#"{"id":"x"}"#, None), &conn).unwrap());
    assert_eq!(401, status);
    let refused = agent.execute(&rpc_request("GetTask", r#"{"id":"x"}"#, Some("wrong")), &conn).unwrap();
    assert_eq!(401, refused.status_code);
    assert!(refused.headers.iter().any(|h| h.name == "WWW-Authenticate"));

    let (_, text) = body(agent.execute(&rpc_request("SendMessage", &message_params("hi", ""), Some("t-alice")), &conn).unwrap());
    let id = s(&json_lite::parse(&text).unwrap(), &["result", "task", "id"]).to_string();
    let (_, mine) = body(agent.execute(&rpc_request("GetTask", &format!(r#"{{"id":"{id}"}}"#), Some("t-alice")), &conn).unwrap());
    assert_eq!(id, s(&json_lite::parse(&mine).unwrap(), &["result", "id"]));
    let (_, theirs) = body(agent.execute(&rpc_request("GetTask", &format!(r#"{{"id":"{id}"}}"#), Some("t-bob")), &conn).unwrap());
    assert_eq!(-32001, error_code(&json_lite::parse(&theirs).unwrap()), "bob can't see alice's task");

    let card = TestClient::new(agent).get(AGENT_CARD_PATH).send();
    assert_eq!(200, card.status(), "the card stays public");
    let card = json_lite::parse(card.body_text()).unwrap();
    assert_eq!("Bearer", s(&card, &["securitySchemes", "bearer", "httpAuthSecurityScheme", "scheme"]));
    assert!(card.get("securityRequirements").and_then(|r| r.as_array()).is_some_and(|r| r.len() == 1));
}

/// A webhook receiver: records each request's headers and body, answers 200.
fn webhook() -> (String, std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}/hooks/", listener.local_addr().unwrap().port());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = std::sync::Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break None;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(p + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&data[..head_end]).to_string();
            let length: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
            while data.len() < head_end + length {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
            }
            log.lock().unwrap().push((head, String::from_utf8_lossy(&data[head_end..]).to_string()));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    (base, seen)
}

#[test]
fn push_notifications_reach_allowed_webhooks() {
    let (base, seen) = webhook();
    let client = TestClient::new(agent().push_notifications([base.as_str()]));
    let config = format!(r#","configuration":{{"taskPushNotificationConfig":{{"url":"{base}task","token":"tok-1","authentication":{{"scheme":"Bearer","credentials":"s3cret"}}}}}}"#);
    let reply = call(&client, "SendMessage", &message_params("slow", &config));
    let id = s(reply.get("result").unwrap().get("task").unwrap(), &["id"]).to_string();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while seen.lock().unwrap().len() < 4 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let seen = seen.lock().unwrap().clone();
    let kinds: Vec<String> = seen.iter().map(|(_, body)| match json_lite::parse(body).unwrap() {
        json_lite::JsonValue::Object(pairs) => pairs[0].0.clone(),
        _ => String::new(),
    }).collect();
    assert_eq!(vec!["task", "statusUpdate", "artifactUpdate", "statusUpdate"], kinds, "{seen:?}");
    let (head, body) = &seen[3];
    assert!(head.starts_with("POST /hooks/task "), "{head}");
    assert!(head.contains("Authorization: Bearer s3cret") && head.contains("X-A2A-Notification-Token: tok-1") && head.contains("Content-Type: application/a2a+json"), "{head}");
    assert_eq!("TASK_STATE_COMPLETED", s(&json_lite::parse(body).unwrap(), &["statusUpdate", "status", "state"]));

    // CRUD; credentials never come back.
    let created = call(&client, "CreateTaskPushNotificationConfig", &format!(r#"{{"taskId":"{id}","id":"c2","url":"{base}two","authentication":{{"scheme":"Basic","credentials":"x"}}}}"#));
    let created = created.get("result").unwrap();
    assert_eq!(("c2", "Basic"), (s(created, &["id"]), s(created, &["authentication", "scheme"])));
    assert!(created.get("authentication").unwrap().get("credentials").is_none());
    assert_eq!("c2", s(call(&client, "GetTaskPushNotificationConfig", &format!(r#"{{"taskId":"{id}","id":"c2"}}"#)).get("result").unwrap(), &["id"]));
    assert_eq!(2, call(&client, "ListTaskPushNotificationConfigs", &format!(r#"{{"taskId":"{id}"}}"#)).get("result").unwrap().get("configs").unwrap().as_array().unwrap().len());
    call(&client, "DeleteTaskPushNotificationConfig", &format!(r#"{{"taskId":"{id}","id":"c2"}}"#));
    call(&client, "DeleteTaskPushNotificationConfig", &format!(r#"{{"taskId":"{id}","id":"c2"}}"#));
    assert_eq!(1, call(&client, "ListTaskPushNotificationConfigs", &format!(r#"{{"taskId":"{id}"}}"#)).get("result").unwrap().get("configs").unwrap().as_array().unwrap().len(), "delete is idempotent");

    // Only listed destinations.
    assert_eq!(-32602, error_code(&call(&client, "CreateTaskPushNotificationConfig", &format!(r#"{{"taskId":"{id}","url":"http://169.254.169.254/latest"}}"#))));
    assert_eq!(-32602, error_code(&call(&client, "SendMessage", &message_params("hi", r#","configuration":{"taskPushNotificationConfig":{"url":"http://10.0.0.1/x"}}"#))));
    assert_eq!(-32001, error_code(&call(&client, "CreateTaskPushNotificationConfig", &format!(r#"{{"taskId":"nope","url":"{base}x"}}"#))));
    let card = json_lite::parse(client.get(AGENT_CARD_PATH).send().body_text()).unwrap();
    assert_eq!(Some(&json_lite::JsonValue::Bool(true)), card.get("capabilities").unwrap().get("pushNotifications"));

    let off = TestClient::new(agent());
    assert_eq!(-32003, error_code(&call(&off, "CreateTaskPushNotificationConfig", r#"{"taskId":"t","url":"https://x/"}"#)));
    assert!(!push::allowed("http://127.0.0.1:1/hooks/x\r\nX: y", &[base.clone()]));
}

#[test]
fn the_extended_card_is_for_authenticated_callers() {
    use crate::application::Application;
    let conn = crate::server::ConnectionInfo::default();
    let agent = agent().require_bearer("t").extended_card(AgentCard::new("Echo agent", "Plus the internal skills", "1.2.0").skill(Skill::new("audit", "Audit", "Internal")));
    let reply = agent.execute(&rpc_request("GetExtendedAgentCard", "{}", Some("t")), &conn).unwrap();
    let reply = json_lite::parse(std::str::from_utf8(&reply.content_range_list[0].body).unwrap()).unwrap();
    assert_eq!("audit", s(&reply.get("result").unwrap().get("skills").unwrap().as_array().unwrap()[0], &["id"]));
    assert_eq!(401, agent.execute(&rpc_request("GetExtendedAgentCard", "{}", None), &conn).unwrap().status_code);
    let card = json_lite::parse(TestClient::new(agent).get(AGENT_CARD_PATH).send().body_text()).unwrap();
    assert_eq!(Some(&json_lite::JsonValue::Bool(true)), card.get("capabilities").unwrap().get("extendedAgentCard"));
    assert_eq!(-32004, error_code(&call(&TestClient::new(super::tests::agent()), "GetExtendedAgentCard", "{}")), "not on the card: unsupported");
}

#[test]
fn trailing_slash_card_caching_and_list_shape() {
    let client = TestClient::new(agent());
    // Clients join "/" to the card's URL: /a2a/ is the endpoint too.
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"GetTask","params":{"id":"x"}}"#;
    let slashed = client.post("/a2a/").header("A2A-Version", "1.0").body_json(body).send();
    assert_eq!((200, Some("application/json")), (slashed.status(), slashed.header("Content-Type").map(|c| c.split(';').next().unwrap())));
    assert_eq!(-32001, error_code(&json_lite::parse(slashed.body_text()).unwrap()));

    let card = client.get(AGENT_CARD_PATH).send();
    let etag = card.header("ETag").expect("ETag").to_string();
    assert!(etag.starts_with('"') && card.header("Last-Modified").is_some_and(|d| d.ends_with("GMT")));
    let again = client.get(AGENT_CARD_PATH).header("If-None-Match", &etag).send();
    assert_eq!((304, ""), (again.status(), again.body_text()));

    send(&client, "hello", r#","contextId":"ctx-shape""#);
    std::thread::sleep(std::time::Duration::from_millis(5));
    send(&client, "later", r#","contextId":"ctx-shape""#);
    let listed = call(&client, "ListTasks", r#"{"contextId":"ctx-shape"}"#);
    let tasks = listed.get("result").unwrap().get("tasks").unwrap().as_array().unwrap();
    assert!(tasks.iter().all(|t| t.get("artifacts").is_none()), "no artifacts without includeArtifacts");
    assert!(s(&tasks[0], &["status", "timestamp"]) >= s(&tasks[1], &["status", "timestamp"]), "newest update first");
    let full = call(&client, "ListTasks", r#"{"contextId":"ctx-shape","includeArtifacts":true}"#);
    assert!(full.get("result").unwrap().get("tasks").unwrap().as_array().unwrap()[0].get("artifacts").is_some());
}

#[test]
fn proto_field_names_work_like_camel_case() {
    let client = TestClient::new(agent());
    let reply = send(&client, "ask", "");
    let id = s(reply.get("result").unwrap().get("task").unwrap(), &["id"]).to_string();
    send(&client, "blue", &format!(r#","task_id":"{id}""#));
    let got = call(&client, "GetTask", &format!(r#"{{"id":"{id}","history_length":1}}"#));
    assert_eq!(1, got.get("result").unwrap().get("history").unwrap().as_array().unwrap().len(), "history_length = historyLength");
    let listed = call(&client, "ListTasks", r#"{"page_size":1}"#);
    assert_eq!(1, listed.get("result").unwrap().get("tasks").unwrap().as_array().unwrap().len());
}

#[test]
fn a_continued_task_notifies_its_webhook() {
    let (base, seen) = webhook();
    let local = base.replace("127.0.0.1", "localhost");
    let client = TestClient::new(agent().push_notifications(["http://localhost:"]));
    let config = format!(r#","configuration":{{"taskPushNotificationConfig":{{"url":"{local}t"}}}}"#);
    let reply = call(&client, "SendMessage", &message_params("ask", &config));
    let id = s(reply.get("result").unwrap().get("task").unwrap(), &["id"]).to_string();
    send(&client, "blue", &format!(r#","taskId":"{id}""#));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while seen.lock().unwrap().len() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let seen = seen.lock().unwrap().clone();
    assert_eq!(2, seen.len(), "{seen:?}");
    assert_eq!("TASK_STATE_INPUT_REQUIRED", s(&json_lite::parse(&seen[0].1).unwrap(), &["task", "status", "state"]));
    assert_eq!("TASK_STATE_COMPLETED", s(&json_lite::parse(&seen[1].1).unwrap(), &["statusUpdate", "status", "state"]));
}
