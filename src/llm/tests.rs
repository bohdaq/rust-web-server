use super::usage::{ChunkDecoder, UsageReader};
use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

fn parse(content_type: &str, body: &str) -> Option<Usage> {
    let mut parser = UsageParser::new(content_type);
    parser.feed(body.as_bytes());
    parser.finish()
}

fn parse_byte_by_byte(content_type: &str, body: &str) -> Option<Usage> {
    let mut parser = UsageParser::new(content_type);
    for b in body.as_bytes() {
        parser.feed(std::slice::from_ref(b));
    }
    parser.finish()
}

const OPENAI_JSON: &str = r#"{"id":"c1","object":"chat.completion","model":"gpt-x","choices":[{"message":{"role":"assistant","content":"hi \"usage\":{\"prompt_tokens\":999}"}}],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17,"prompt_tokens_details":{"cached_tokens":0}}}"#;

const OPENAI_SSE: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"He\"}}],\"usage\":null}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"llo\"}}],\"usage\":null}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":9,\"completion_tokens\":2,\"total_tokens\":11}}\n\n\
data: [DONE]\n\n";

const ANTHROPIC_SSE: &str = "event: message_start\r\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"claude-x\",\"usage\":{\"input_tokens\":25,\"cache_read_input_tokens\":100,\"cache_creation_input_tokens\":0,\"output_tokens\":1}}}\r\n\r\n\
event: content_block_delta\r\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\r\n\r\n\
event: message_delta\r\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":15}}\r\n\r\n\
event: message_stop\r\n\
data: {\"type\":\"message_stop\"}\r\n\r\n";

#[test]
fn usage_from_an_openai_json_body_ignores_lookalikes_inside_content() {
    assert_eq!(parse("application/json", OPENAI_JSON), Some(Usage { input_tokens: 12, output_tokens: 5 }));
}

#[test]
fn usage_from_an_openai_stream_comes_from_the_final_chunk() {
    let expected = Some(Usage { input_tokens: 9, output_tokens: 2 });
    assert_eq!(parse("text/event-stream; charset=utf-8", OPENAI_SSE), expected);
    assert_eq!(parse_byte_by_byte("text/event-stream", OPENAI_SSE), expected);
}

#[test]
fn usage_from_an_anthropic_stream_counts_cache_tokens_as_input_and_the_last_output_count() {
    let expected = Some(Usage { input_tokens: 125, output_tokens: 15 });
    assert_eq!(parse("text/event-stream", ANTHROPIC_SSE), expected);
    assert_eq!(parse_byte_by_byte("text/event-stream", ANTHROPIC_SSE), expected);
}

#[test]
fn usage_from_anthropic_json_and_the_responses_api() {
    let anthropic = r#"{"id":"m","type":"message","content":[{"type":"text","text":"x"}],"usage":{"input_tokens":3,"output_tokens":4}}"#;
    assert_eq!(parse("application/json", anthropic), Some(Usage { input_tokens: 3, output_tokens: 4 }));
    let responses = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"usage\":{\"input_tokens\":7,\"output_tokens\":8,\"total_tokens\":15}}}\n\n";
    assert_eq!(parse("text/event-stream", responses), Some(Usage { input_tokens: 7, output_tokens: 8 }));
}

#[test]
fn no_usage_bad_json_and_oversized_lines_give_nothing() {
    assert_eq!(parse("application/json", r#"{"data":[{"id":"gpt-x"}]}"#), None);
    assert_eq!(parse("application/json", "not json"), None);
    assert_eq!(parse("text/event-stream", "data: {\"usage\":{\"prompt_tokens\":\"x\"}}\n"), None);
    let huge = format!("data: {{\"pad\":\"{}\",\"usage\":{{\"prompt_tokens\":1}}}}\n", "a".repeat(2 * 1024 * 1024));
    let mut parser = UsageParser::new("text/event-stream");
    for piece in huge.as_bytes().chunks(4096) {
        parser.feed(piece);
    }
    parser.feed(b"data: {\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3}}\n");
    assert_eq!(parser.finish(), Some(Usage { input_tokens: 2, output_tokens: 3 }), "the line past the cap is skipped, the next one read");
}

#[test]
fn chunk_decoder_handles_any_split_extensions_and_trailers() {
    let framed = b"4;ext=1\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\nX-Trailer: y\r\n\r\n";
    for split in 1..framed.len() {
        let mut decoder = ChunkDecoder::default();
        let mut out = Vec::new();
        decoder.decode(&framed[..split], &mut out).unwrap();
        decoder.decode(&framed[split..], &mut out).unwrap();
        assert_eq!(out, b"Wikipedia in\r\n\r\nchunks.", "split at {split}");
        assert!(decoder.done());
    }
    let mut out = Vec::new();
    assert!(ChunkDecoder::default().decode(b"zz\r\n", &mut out).is_err());
    assert!(ChunkDecoder::default().decode(b"2\r\nabc\r\n", &mut out).is_err());
    assert!(ChunkDecoder::default().decode(&vec![b'1'; 9000], &mut out).is_err());
}

#[test]
fn usage_reader_unframes_counts_and_reports_once_even_when_dropped_early() {
    let body = ANTHROPIC_SSE.as_bytes();
    let mut framed = Vec::new();
    for piece in body.chunks(37) {
        framed.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
        framed.extend_from_slice(piece);
        framed.extend_from_slice(b"\r\n");
    }
    framed.extend_from_slice(b"0\r\n\r\n");
    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();
    let mut reader = UsageReader::new(Box::new(std::io::Cursor::new(framed)), "text/event-stream", true, Instant::now(), Box::new(move |u, t| tx.send((u, t.is_some())).unwrap()));
    let mut out = Vec::new();
    let mut buf = [0u8; 10];
    loop {
        let n = reader.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    assert_eq!(out, body);
    drop(reader);
    assert_eq!(rx.try_recv().unwrap(), (Some(Usage { input_tokens: 125, output_tokens: 15 }), true));
    assert!(rx.try_recv().is_err(), "reported once");

    // A client that goes away after the first event: still reported.
    let mut reader = UsageReader::new(Box::new(std::io::Cursor::new(body.to_vec())), "text/event-stream", false, Instant::now(), Box::new(move |u, _| tx2.send((u, true)).unwrap()));
    let mut first = vec![0u8; 300];
    let _ = reader.read(&mut first).unwrap();
    drop(reader);
    assert_eq!(rx.try_recv().unwrap().0, Some(Usage { input_tokens: 125, output_tokens: 1 }));
}

#[test]
fn request_fields_are_read_at_the_top_level_only() {
    let a = asked(br#"{"messages":[{"role":"user","content":"{\"model\":\"evil\"}"}],"model":"gpt-4o","stream":true}"#);
    assert_eq!((a.model.as_str(), a.stream, a.has_stream_options), ("gpt-4o", true, false));
    let a = asked(br#"{"model":"m","stream":false,"stream_options":{"include_usage":false}}"#);
    assert_eq!((a.stream, a.has_stream_options), (false, true));
    assert_eq!(asked(b"garbage").model, "");
}

#[test]
fn the_usage_option_is_added_as_the_first_member() {
    assert_eq!(with_usage_option(br#" {"model":"m"}"#).unwrap(), br#"{"stream_options":{"include_usage":true},"model":"m"}"#);
    assert_eq!(with_usage_option(b"{ }").unwrap(), br#"{"stream_options":{"include_usage":true} }"#);
    assert_eq!(with_usage_option(b"[1]"), None);
}

#[test]
fn apis_and_provider_urls() {
    assert_eq!(Api::of_path("/v1/messages"), Api::Anthropic);
    assert_eq!(Api::of_path("/v1/messages/count_tokens?x=1"), Api::Anthropic);
    assert_eq!(Api::of_path("/v1/chat/completions"), Api::OpenAi);
    assert_eq!(Api::parse("OpenAI"), Some(Api::OpenAi));
    assert_eq!(Api::parse("gemini"), None);
    let p = Provider::new("o", Api::OpenAi, "https://api.openai.com").unwrap();
    assert_eq!((p.tls, p.host.as_str(), p.port, p.base_path.as_str()), (true, "api.openai.com", 443, ""));
    let p = Provider::new("l", Api::OpenAi, "http://127.0.0.1:8080/openai/").unwrap();
    assert_eq!((p.tls, p.host.as_str(), p.port, p.base_path.as_str()), (false, "127.0.0.1", 8080, "/openai"));
    let p = Provider::new("v6", Api::OpenAi, "http://[::1]:9/").unwrap();
    assert_eq!((p.host.as_str(), p.port), ("::1", 9));
    assert!(Provider::new("x", Api::OpenAi, "ftp://x").is_err());
    assert!(Provider::new("x", Api::OpenAi, "http://h:port").is_err());
    let shown = format!("{:?}", Provider::new("k", Api::Anthropic, "https://a").unwrap().api_key("sk-secret"));
    assert!(!shown.contains("sk-secret"), "{shown}");
}

/// A provider stand-in: answers each request with `reply(head, body)` and
/// sends what it received over the channel.
fn provider(reply: impl Fn(&str, &str) -> String + Send + 'static) -> (String, mpsc::Receiver<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
                head.push_str(&line);
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let body = String::from_utf8_lossy(&body).to_string();
            let _ = stream.write_all(reply(&head, &body).as_bytes());
            let _ = tx.send((head, body));
        }
    });
    (url, rx)
}

fn json_reply(status: &str, body: &str) -> String {
    format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn post(app: &LlmProxy, path: &str, key: &str, body: &str) -> Response {
    let raw = format!("POST {path} HTTP/1.1\r\nHost: llm\r\nAuthorization: Bearer {key}\r\nAccept-Encoding: gzip\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    app.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap()
}

fn body_of(mut response: Response) -> String {
    match response.stream_pipe.take() {
        Some(mut pipe) => {
            let mut out = String::new();
            pipe.read_to_string(&mut out).unwrap();
            out
        }
        None => response.content_range_list.first().map(|p| String::from_utf8_lossy(&p.body).to_string()).unwrap_or_default(),
    }
}

#[test]
fn keys_are_swapped_providers_fail_over_and_budgets_stop_spenders() {
    let (busy, busy_seen) = provider(|_, _| json_reply("429 Too Many Requests", r#"{"error":"slow down"}"#));
    let (good, good_seen) = provider(|_, _| json_reply("200 OK", r#"{"choices":[],"usage":{"prompt_tokens":60,"completion_tokens":50}}"#));
    let app = LlmProxy::new()
        .provider(Provider::new("busy", Api::OpenAi, &busy).unwrap().api_key("sk-busy"))
        .provider(Provider::new("good", Api::OpenAi, &format!("{good}/base")).unwrap().api_key("sk-good"))
        .virtual_key("vk-alice", "alice")
        .budget("alice", 200, 3600);

    assert_eq!(post(&app, "/v1/chat/completions", "vk-eve", "{}").status_code, 401);

    let response = post(&app, "/v1/chat/completions", "vk-alice", r#"{"model":"gpt-x","messages":[]}"#);
    assert_eq!(response.status_code, 200);
    assert!(body_of(response).contains("prompt_tokens"));
    let (busy_head, _) = busy_seen.recv().unwrap();
    let (good_head, good_body) = good_seen.recv().unwrap();
    assert!(busy_head.contains("Bearer sk-busy"), "{busy_head}");
    assert!(good_head.starts_with("POST /base/v1/chat/completions "), "{good_head}");
    assert!(good_head.contains("Authorization: Bearer sk-good"), "{good_head}");
    let lower = good_head.to_ascii_lowercase();
    assert!(!lower.contains("vk-alice") && !lower.contains("accept-encoding"), "{good_head}");
    assert_eq!(good_body, r#"{"model":"gpt-x","messages":[]}"#, "non-streaming bodies are sent unchanged");

    // 110 tokens spent of 200: one more request is allowed, then 429.
    assert_eq!(post(&app, "/v1/chat/completions", "vk-alice", r#"{"model":"gpt-x"}"#).status_code, 200);
    let refused = post(&app, "/v1/chat/completions", "vk-alice", r#"{"model":"gpt-x"}"#);
    assert_eq!(refused.status_code, 429);
    assert!(refused.headers.iter().any(|h| h.name.eq_ignore_ascii_case("retry-after")));

    let metrics = crate::metrics::llm::prometheus_text();
    assert!(metrics.contains(r#"rws_llm_tokens_total{provider="good",model="gpt-x",identity="alice",type="input"} 120"#), "{metrics}");
    assert!(metrics.contains(r#"rws_llm_fallbacks_total{provider="busy"}"#), "{metrics}");
    assert!(metrics.contains(r#"rws_llm_budget_rejections_total{identity="alice"}"#), "{metrics}");
}

#[test]
fn streams_are_counted_as_they_pass_and_openai_streams_ask_for_usage() {
    let sse = OPENAI_SSE.to_string();
    let (url, seen) = provider(move |_, _| {
        let mut out = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_string();
        for piece in sse.as_bytes().chunks(20) {
            out.push_str(&format!("{:x}\r\n{}\r\n", piece.len(), String::from_utf8_lossy(piece)));
        }
        out.push_str("0\r\n\r\n");
        out
    });
    let (anthropic, anthropic_seen) = provider(|_, _| json_reply("200 OK", r#"{"type":"message","usage":{"input_tokens":4,"output_tokens":6}}"#));
    let app = LlmProxy::new()
        .provider(Provider::new("stream", Api::OpenAi, &url).unwrap().api_key("sk-s"))
        .provider(Provider::new("claude", Api::Anthropic, &anthropic).unwrap().api_key("sk-ant"))
        .virtual_key("vk-bob", "bob")
        .budget("bob", 1_000, 3600);

    let response = post(&app, "/v1/chat/completions", "vk-bob", r#"{"model":"gpt-s","stream":true}"#);
    assert!(!response.headers.iter().any(|h| h.name.eq_ignore_ascii_case("transfer-encoding")), "the server frames the body again");
    assert_eq!(body_of(response), OPENAI_SSE, "the client gets the events unframed");
    let (_, body) = seen.recv().unwrap();
    assert_eq!(body, r#"{"stream_options":{"include_usage":true},"model":"gpt-s","stream":true}"#);
    let metrics = crate::metrics::llm::prometheus_text();
    assert!(metrics.contains(r#"rws_llm_tokens_total{provider="stream",model="gpt-s",identity="bob",type="output"} 2"#), "{metrics}");
    assert!(metrics.contains(r#"rws_llm_time_to_first_token_seconds_count{provider="stream",model="gpt-s"} 1"#), "{metrics}");

    let raw = "POST /v1/messages HTTP/1.1\r\nHost: llm\r\nx-api-key: vk-bob\r\nContent-Type: application/json\r\nContent-Length: 15\r\n\r\n{\"model\":\"c-1\"}";
    let response = app.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
    assert_eq!(response.status_code, 200);
    let (head, _) = anthropic_seen.recv().unwrap();
    assert!(head.contains("x-api-key: sk-ant") && head.contains("anthropic-version: 2023-06-01"), "{head}");
    assert!(!head.contains("vk-bob"), "{head}");
}

#[test]
fn no_answer_is_a_502_and_an_api_without_providers_a_404() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let app = LlmProxy::new()
        .provider(Provider::new("dead", Api::OpenAi, &dead).unwrap())
        .allow_anonymous()
        .timeouts(Duration::from_millis(500), Duration::from_millis(500));
    assert_eq!(post(&app, "/v1/chat/completions", "", "{}").status_code, 502);
    assert_eq!(post(&app, "/v1/messages", "", "{}").status_code, 404);
}

/// A Redis stand-in that understands GET, INCRBY and EXPIRE; records them.
fn fake_redis() -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let log = std::sync::Arc::new(Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&log);
    let data: std::sync::Arc<Mutex<HashMap<String, i64>>> = std::sync::Arc::default();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let (seen, data) = (std::sync::Arc::clone(&seen), std::sync::Arc::clone(&data));
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    let n: usize = line.trim().trim_start_matches('*').parse().unwrap();
                    let mut args = Vec::new();
                    for _ in 0..n {
                        let mut len = String::new();
                        reader.read_line(&mut len).unwrap();
                        let mut arg = String::new();
                        reader.read_line(&mut arg).unwrap();
                        args.push(arg.trim_end().to_string());
                    }
                    seen.lock().unwrap().push(args.join(" "));
                    let mut data = data.lock().unwrap();
                    let reply = match args[0].as_str() {
                        "GET" => match data.get(&args[1]) {
                            Some(v) => format!("${}\r\n{v}\r\n", v.to_string().len()),
                            None => "$-1\r\n".to_string(),
                        },
                        "INCRBY" => {
                            let v = data.entry(args[1].clone()).or_default();
                            *v += args[2].parse::<i64>().unwrap();
                            format!(":{v}\r\n")
                        }
                        _ => ":1\r\n".to_string(),
                    };
                    stream.write_all(reply.as_bytes()).unwrap();
                }
            });
        }
    });
    (addr, log)
}

#[test]
fn budgets_in_redis_are_shared_by_every_proxy_pointing_at_it() {
    let (redis, commands) = fake_redis();
    let (url, _seen) = provider(|_, _| json_reply("200 OK", r#"{"usage":{"prompt_tokens":40,"completion_tokens":40}}"#));
    let replica = || {
        LlmProxy::new()
            .provider(Provider::new("p", Api::OpenAi, &url).unwrap())
            .virtual_key("vk-dan", "dan")
            .budget("dan", 100, 3600)
            .redis(redis.clone(), None, "test:")
    };
    let (one, two) = (replica(), replica());
    assert_eq!(post(&one, "/v1/chat/completions", "vk-dan", "{}").status_code, 200);
    assert_eq!(post(&two, "/v1/chat/completions", "vk-dan", "{}").status_code, 200, "80 of 100 spent");
    assert_eq!(post(&one, "/v1/chat/completions", "vk-dan", "{}").status_code, 429, "160 spent, counted across both");
    let commands = commands.lock().unwrap().clone();
    let window = now_secs() / 3600;
    let key = format!("test:dan:3600:{window}");
    assert!(commands.contains(&format!("INCRBY {key} 80")), "{commands:?}");
    assert_eq!(commands.iter().filter(|c| c.starts_with("EXPIRE")).count(), 1, "expiry set on the first charge only: {commands:?}");
}

#[test]
fn an_unreachable_redis_lets_requests_through() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let redis = closed.local_addr().unwrap().to_string();
    drop(closed);
    let (url, _seen) = provider(|_, _| json_reply("200 OK", r#"{"usage":{"prompt_tokens":500}}"#));
    let app = LlmProxy::new().provider(Provider::new("p", Api::OpenAi, &url).unwrap()).virtual_key("k", "erin").budget("erin", 10, 60).redis(redis, None, "x:");
    for _ in 0..3 {
        assert_eq!(post(&app, "/v1/chat/completions", "k", "{}").status_code, 200);
    }
}
