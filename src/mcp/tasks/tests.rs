use super::*;
use crate::application::Application;
use crate::mcp::InputRequired;
use crate::request::Request;

const V: &str = "2026-07-28";

fn server() -> McpServer {
    McpServer::new("tasks-test", "1.0")
        .tool_with_context("slow", "Sleeps then answers", r#"{"type":"object"}"#, |ctx, _| {
            ctx.start_task(|task| {
                for _ in 0..50 {
                    if task.is_cancelled() {
                        return Err(TaskError::Cancelled);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(McpContent::text("slow done"))
            })
        })
        .tool_with_context("ask_two", "Asks two things in the task", r#"{"type":"object"}"#, |ctx, _| {
            ctx.start_task(|task| {
                let answers = task.input(InputRequired::new().elicit("a", "A?", "{}").elicit("b", "B?", "{}"))?;
                Ok(McpContent::text(format!("{} answers", answers.len())))
            })
        })
        .tool_with_context("broken", "Fails at protocol level", r#"{"type":"object"}"#, |ctx, _| {
            ctx.start_task(|_task| Err(TaskError::Failed { code: -32603, message: "boom".to_string() }))
        })
        .tool_with_context("panics", "Panics", r#"{"type":"object"}"#, |ctx, _| ctx.start_task(|_task| panic!("in the task")))
        .tool_with_context("tool_error", "A tool error", r#"{"type":"object"}"#, |ctx, _| ctx.start_task(|_task| Err("nope".into())))
        .task_required("ask_two")
}

fn connection() -> crate::server::ConnectionInfo {
    crate::server::ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 10_000,
        sni_hostname: None,
    }
}

/// A modern request to `server`; `tasks` declares the extension.
fn call(server: &McpServer, method: &str, name: Option<&str>, params: &str, tasks: bool) -> (u16, String) {
    let capabilities = if tasks { r#"{"elicitation":{},"extensions":{"io.modelcontextprotocol/tasks":{}}}"# } else { "{}" };
    let meta = format!(r#""_meta":{{"io.modelcontextprotocol/protocolVersion":"{V}","io.modelcontextprotocol/clientCapabilities":{capabilities}}}"#);
    let comma = if params.is_empty() { "" } else { "," };
    let mut headers = vec![("Content-Type", "application/json"), ("MCP-Protocol-Version", V), ("Mcp-Method", method)];
    if let Some(name) = name {
        headers.push(("Mcp-Name", name));
    }
    let request = Request {
        method: "POST".to_string(),
        request_uri: "/mcp".to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| crate::header::Header { name: n.to_string().into(), value: v.to_string().into() }).collect(),
        body: format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{{{params}{comma}{meta}}}}}"#).into_bytes().into(),
    };
    let response = server.execute(&request, &connection()).unwrap();
    let body = response.content_range_list.first().map(|c| String::from_utf8_lossy(&c.body).into_owned()).unwrap_or_default();
    (response.status_code as u16, body)
}

fn start(server: &McpServer, tool: &str) -> String {
    let (status, body) = call(server, "tools/call", Some(tool), &format!(r#""name":"{tool}","arguments":{{}}"#), true);
    assert_eq!(200, status, "{body}");
    assert!(body.contains(r#""resultType":"task""#) && body.contains(r#""status":"working""#), "{body}");
    assert!(!body.contains("requestState") && !body.contains(r#""task":{"#), "a flat CreateTaskResult: {body}");
    json_rpc::extract_str(&body, "taskId").expect("a taskId")
}

fn get(server: &McpServer, id: &str) -> String {
    call(server, "tasks/get", Some(id), &format!(r#""taskId":"{id}""#), true).1
}

/// Poll `tasks/get` until the status is `status` (or give up).
fn wait_for(server: &McpServer, id: &str, status: &str) -> String {
    for _ in 0..200 {
        let body = get(server, id);
        if body.contains(&format!(r#""status":"{status}""#)) {
            return body;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("task {id} never reached {status}: {}", get(server, id));
}

#[test]
fn a_task_runs_to_completion_and_inlines_its_result() {
    let server = server();
    let id = start(&server, "slow");
    let done = wait_for(&server, &id, "completed");
    assert!(done.contains(r#""resultType":"complete""#) && done.contains(r#""result":{"content":[{"type":"text","text":"slow done"}],"isError":false}"#), "{done}");
    assert!(done.contains(r#""ttlMs":3600000"#) && done.contains(r#""pollIntervalMs":1000"#), "{done}");
}

#[test]
fn without_the_extension_the_work_runs_inline() {
    let (status, body) = call(&server(), "tools/call", Some("slow"), r#""name":"slow","arguments":{}"#, false);
    assert_eq!(200, status);
    assert!(body.contains(r#""resultType":"complete""#) && body.contains("slow done") && !body.contains("taskId"), "{body}");
}

#[test]
fn task_required_tools_and_the_task_methods_need_the_extension() {
    let server = server();
    let (status, body) = call(&server, "tools/call", Some("ask_two"), r#""name":"ask_two","arguments":{}"#, false);
    assert_eq!(400, status);
    assert!(body.contains(r#""code":-32021"#) && body.contains(r#""requiredCapabilities":{"extensions":{"io.modelcontextprotocol/tasks":{}}}"#), "{body}");
    let (status, body) = call(&server, "tasks/get", Some("x"), r#""taskId":"x""#, false);
    assert_eq!((400, true), (status, body.contains("-32021")), "{body}");
    let (_, body) = call(&server, "tasks/get", Some("unknown"), r#""taskId":"unknown""#, true);
    assert!(body.contains(r#""code":-32602"#), "{body}");
    let (status, body) = call(&server, "tasks/get", Some("other"), r#""taskId":"x""#, true);
    assert_eq!((400, true), (status, body.contains("-32020")), "Mcp-Name is the taskId: {body}");
}

#[test]
fn input_is_answered_through_tasks_update_one_key_at_a_time() {
    let server = server();
    let id = start(&server, "ask_two");
    let parked = wait_for(&server, &id, "input_required");
    assert!(parked.contains(r#""inputRequests":{"a":{"method":"elicitation/create""#) && parked.contains(r#""b":{"method""#), "{parked}");
    let update = |responses: &str| call(&server, "tasks/update", Some(&id), &format!(r#""taskId":"{id}","inputResponses":{responses}"#), true).1;
    let ack = update(r#"{"a":{"action":"accept","content":{}},"zzz":{"action":"accept"}}"#);
    assert!(ack.contains(r#""result":{"resultType":"complete""#) && !ack.contains("taskId"), "an empty ack: {ack}");
    let partial = get(&server, &id);
    assert!(partial.contains(r#""status":"input_required""#) && !partial.contains(r#""a":{"#) && partial.contains(r#""b":{"#), "{partial}");
    update(r#"{"b":{"action":"accept","content":{}}}"#);
    let done = wait_for(&server, &id, "completed");
    assert!(done.contains("2 answers"), "{done}");
}

#[test]
fn cancelling_settles_on_cancelled_and_is_idempotent() {
    let server = server();
    let id = start(&server, "slow");
    let cancel = || call(&server, "tasks/cancel", Some(&id), &format!(r#""taskId":"{id}""#), true).1;
    assert!(cancel().contains(r#""resultType":"complete""#));
    let cancelled = get(&server, &id);
    assert!(cancelled.contains(r#""status":"cancelled""#) && !cancelled.contains(r#""result":{"content""#), "{cancelled}");
    std::thread::sleep(Duration::from_millis(600));
    assert!(get(&server, &id).contains(r#""status":"cancelled""#), "the finished work doesn't overwrite it");
    assert!(cancel().contains(r#""resultType":"complete""#), "cancelling a finished task is acknowledged too");
}

#[test]
fn tool_errors_complete_and_protocol_errors_fail() {
    let server = server();
    let tool_error = wait_for(&server, &start(&server, "tool_error"), "completed");
    assert!(tool_error.contains(r#""isError":true"#), "{tool_error}");
    let failed = wait_for(&server, &start(&server, "broken"), "failed");
    assert!(failed.contains(r#""error":{"code":-32603,"message":"boom"}"#) && !failed.contains(r#""result":{"content""#), "{failed}");
    let panicked = wait_for(&server, &start(&server, "panics"), "failed");
    assert!(panicked.contains(r#""code":-32603"#), "{panicked}");
}

#[test]
fn a_task_belongs_to_its_caller() {
    let server = server();
    let id = start(&server, "slow");
    assert!(server.task_get(&id, "").is_ok());
    assert!(server.task_get(&id, "someone-else").is_err());
    assert!(server.task_cancel(&id, "someone-else").is_err());
}

#[test]
fn listen_streams_status_changes_of_its_tasks() {
    use std::io::Read;
    let server = server();
    let id = start(&server, "slow");
    let mut stream = server.listen("9", &format!(r#"{{"notifications":{{"taskIds":["{id}"]}}}}"#), true);
    let mut text = String::new();
    let reader = stream.stream_pipe.as_mut().unwrap();
    while !text.contains(r#""status":"completed""#) {
        let mut buf = [0u8; 4096];
        let n = reader.read(&mut buf).unwrap();
        text.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    assert!(text.contains(&format!(r#""taskIds":["{id}"]"#)), "{text}");
    assert!(text.contains("notifications/tasks") && text.contains(r#"subscriptionId":9"#), "{text}");
    let refused = server.listen("10", r#"{"notifications":{"taskIds":["x"]}}"#, false);
    assert_eq!(400, refused.status_code);
}

#[test]
fn running_tasks_are_capped() {
    let server = server().max_running_tasks(1);
    let first = start(&server, "slow");
    let (_, refused) = call(&server, "tools/call", Some("slow"), r#""name":"slow","arguments":{}"#, true);
    assert!(refused.contains(r#""isError":true"#) && refused.contains("too many tasks running"), "{refused}");
    call(&server, "tasks/cancel", Some(&first), &format!(r#""taskId":"{first}""#), true);
    // The slot frees when the cancelled work actually stops.
    for _ in 0..100 {
        let (_, body) = call(&server, "tools/call", Some("slow"), r#""name":"slow","arguments":{}"#, true);
        if body.contains(r#""resultType":"task""#) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the slot never freed");
}

#[test]
fn timestamps_are_iso_8601() {
    assert_eq!("1970-01-01T00:00:00.000Z", iso8601(0));
    assert_eq!("2026-07-28T10:30:00.123Z", iso8601(1_785_234_600_123));
}
