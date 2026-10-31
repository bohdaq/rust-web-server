//! The agent the official A2A compatibility kit (a2aproject/a2a-tck) runs
//! against: each behaviour of the kit's scenarios (`scenarios/*.feature`),
//! chosen by the message id's prefix.
//!
//! ```text
//! RWS_CONFIG_PORT=9999 cargo run --example a2a_tck --features http2 &
//! ./run_tck.py --sut-host http://127.0.0.1:9999 --transport jsonrpc
//! ```
//!
//! `ci/e2e/a2a_tck.sh` does both; known failures are in
//! `ci/a2a-tck-baseline.txt`.

use std::time::Duration;

use rust_web_server::a2a::{A2aReply, A2aServer, AgentCard, Artifact, Message, Part, Skill};
use rust_web_server::server::Server;

/// `TCK_STREAMING_TIMEOUT` (seconds, default 2): the resubscription task
/// runs for twice that.
fn streaming_timeout() -> Duration {
    let secs = std::env::var("TCK_STREAMING_TIMEOUT").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(2.0);
    Duration::from_secs_f64(secs.max(0.1))
}

pub fn agent() -> A2aServer {
    let card = AgentCard::new("rws A2A TCK agent", "The A2A compatibility kit's scenarios, served by rws", env!("CARGO_PKG_VERSION"))
        .provider("rws", "https://rws8.tech")
        .modes(&["text"], &["text"])
        .skill(Skill::new("tck", "TCK Conformance", "Handles TCK conformance test messages").tag("tck"));
    A2aServer::new(card)
        // The kit's webhook receiver runs on this machine. Loopback is allowed
        // here only for the test run: a real agent lists its partners' hooks.
        .push_notifications(["http://localhost:", "http://127.0.0.1:"])
        .on_message(|ctx, message| {
        let id = message.message_id.clone();
        let is = |prefix: &str| id.starts_with(prefix);
        if is("tck-message-response") {
            return A2aReply::Message(Message::agent_text("Direct message response"));
        }
        if is("tck-input-required") {
            return A2aReply::Task(ctx.input_required("More input, please."));
        }
        if is("tck-reject-task") {
            return A2aReply::Task(ctx.reject("rejected"));
        }
        if is("tck-artifact-file-url") {
            return A2aReply::Task(ctx.complete(vec![Artifact::new("output", vec![Part::url("https://example.com/output.txt", "text/plain").filename("output.txt")])]));
        }
        if is("tck-artifact-text") {
            return A2aReply::Task(ctx.complete(vec![Artifact::text("output", "Generated text content")]));
        }
        if is("tck-artifact-file") {
            return A2aReply::Task(ctx.complete(vec![Artifact::new("output", vec![Part::raw(b"tck".to_vec(), "text/plain").filename("output.txt")])]));
        }
        if is("tck-artifact-data") {
            return A2aReply::Task(ctx.complete(vec![Artifact::new("output", vec![Part::data(r#"{"key":"value","count":42}"#)])]));
        }
        // Streamed lifecycles: working, an artifact, completed.
        let streamed: Option<Vec<(Artifact, bool, bool)>> = if is("tck-stream-artifact-chunked") {
            let first = Artifact::text("output", "chunk-1 ");
            let mut second = Artifact::text("output", "chunk-2");
            second.artifact_id = first.artifact_id.clone();
            Some(vec![(first, true, false), (second, true, true)])
        } else if is("tck-stream-artifact-file") {
            Some(vec![(Artifact::new("output", vec![Part::raw(b"tck".to_vec(), "text/plain").filename("output.txt")]), false, true)])
        } else if is("tck-stream-artifact-text") {
            Some(vec![(Artifact::text("output", "Streamed text content"), false, true)])
        } else if is("tck-stream-ordering-001") {
            Some(vec![(Artifact::text("output", "Ordered output"), false, true)])
        } else if is("tck-stream-001") {
            Some(vec![(Artifact::text("output", "Stream hello from TCK"), false, true)])
        } else if is("tck-stream-003") {
            Some(vec![(Artifact::text("output", "Stream task lifecycle"), false, true)])
        } else if is("tck-stream-002") {
            Some(Vec::new())
        } else {
            None
        };
        if let Some(artifacts) = streamed {
            let task = ctx.start();
            let worker = task.clone();
            std::thread::spawn(move || {
                worker.working(None);
                for (artifact, append, last) in artifacts {
                    worker.artifact_chunk(artifact, append, last);
                }
                worker.complete();
            });
            return A2aReply::Running(task);
        }
        if is("test-resubscribe-message-id") {
            let task = ctx.start();
            let worker = task.clone();
            std::thread::spawn(move || {
                worker.working(None);
                std::thread::sleep(streaming_timeout() * 2);
                worker.complete();
            });
            return A2aReply::Running(task);
        }
        if is("tck-complete-task") {
            return A2aReply::Task(ctx.task(rust_web_server::a2a::TaskState::Completed, Some(Message::agent_text("Hello from TCK")), Vec::new()));
        }
        A2aReply::Task(ctx.task(
            rust_web_server::a2a::TaskState::Completed,
            Some(Message::agent_text(format!("Unhandled messageId prefix: {id}"))),
            Vec::new(),
        ))
    })
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, agent()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn the_scenarios_answer() {
        let client = TestClient::new(agent());
        let send = |id: &str| {
            let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{{"message":{{"messageId":"{id}","role":"ROLE_USER","parts":[{{"text":"hi"}}]}}}}}}"#);
            client.post("/a2a").header("A2A-Version", "1.0").body_json(&body).send().body_text().to_string()
        };
        assert!(send("tck-complete-task-1").contains("Hello from TCK"));
        assert!(send("tck-message-response-1").contains(r#""message":"#));
        assert!(send("tck-input-required-1").contains("TASK_STATE_INPUT_REQUIRED"));
        assert!(send("tck-reject-task-1").contains("TASK_STATE_REJECTED"));
        assert!(send("tck-artifact-data-1").contains(r#""data":{"key":"value","count":42}"#));
        let chunked = send("tck-stream-artifact-chunked-1");
        assert!(chunked.contains("TASK_STATE_COMPLETED") && chunked.contains("chunk-1 ") && chunked.contains("chunk-2"), "{chunked}");
    }
}
