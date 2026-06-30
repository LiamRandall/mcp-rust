//! Streamable-HTTP SSE responses: notification streaming (logging / progress)
//! and server→client request orchestration (sampling / elicitation).

use serde_json::{json, Value};

use crate::wasi::clocks::monotonic_clock;
use crate::wasi::http::types::{Fields, OutgoingBody, OutgoingResponse, ResponseOutparam};
use crate::wasi::io::streams::{OutputStream, StreamError};

pub enum Note {
    Log { level: String, data: Value },
    Progress { token: Value, progress: f64, total: f64 },
    Delay(u64),
}

pub enum Kind {
    Sampling,
    Elicitation,
}

pub enum Plan {
    /// Emit notifications, then the final JSON-RPC result for `id`.
    Notify {
        id: Value,
        notes: Vec<Note>,
        result: Value,
    },
    /// Emit a server→client request, await the client's response (correlated
    /// through keyvalue), then emit the final result for `id`.
    Callback {
        id: Value,
        request: Value,
        corr_id: String,
        kind: Kind,
    },
}

const POLL_INTERVAL_MS: u64 = 25;
const CALLBACK_TIMEOUT_MS: u64 = 15_000;

pub fn stream(out: ResponseOutparam, plan: Plan) {
    let headers = Fields::new();
    let _ = headers.append(&"content-type".to_string(), b"text/event-stream");
    let _ = headers.append(&"cache-control".to_string(), b"no-cache");
    let resp = OutgoingResponse::new(headers);
    let _ = resp.set_status_code(200);
    let body = resp.body().expect("outgoing body");
    ResponseOutparam::set(out, Ok(resp));

    {
        let w = body.write().expect("outgoing stream");
        match plan {
            Plan::Notify { id, notes, result } => {
                for note in notes {
                    match note {
                        Note::Delay(ms) => sleep_ms(ms),
                        Note::Log { level, data } => {
                            send_event(&w, &json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/message",
                                "params": { "level": level, "logger": "mcp-rust", "data": data }
                            }));
                        }
                        Note::Progress { token, progress, total } => {
                            send_event(&w, &json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/progress",
                                "params": { "progressToken": token, "progress": progress, "total": total }
                            }));
                        }
                    }
                }
                send_event(&w, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
            }
            Plan::Callback { id, request, corr_id, kind } => {
                send_event(&w, &request);
                let response = await_response(&corr_id);
                let result = build_callback_result(kind, response);
                send_event(&w, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
            }
        }
        drop(w);
    }
    let _ = OutgoingBody::finish(body, None);
}

fn build_callback_result(kind: Kind, response: Option<Value>) -> Value {
    match (kind, response) {
        (Kind::Sampling, Some(resp)) => {
            let text = resp
                .get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            json!({ "content": [ { "type": "text", "text": format!("LLM response: {text}") } ] })
        }
        (Kind::Elicitation, Some(resp)) => {
            let result = resp.get("result").cloned().unwrap_or(Value::Null);
            let action = result.get("action").and_then(|a| a.as_str()).unwrap_or("unknown");
            let content = result.get("content").cloned().unwrap_or(json!({}));
            json!({ "content": [ { "type": "text",
                "text": format!("User response: action={action}, content={content}") } ] })
        }
        (_, None) => json!({
            "content": [ { "type": "text", "text": "No response from client (timed out)." } ],
            "isError": true
        }),
    }
}

/// Poll the shared keyvalue store for the correlated client response.
fn await_response(corr_id: &str) -> Option<Value> {
    let key = format!("mcpresp:{corr_id}");
    let mut waited = 0u64;
    while waited < CALLBACK_TIMEOUT_MS {
        if let Some(bytes) = crate::kv::take(&key) {
            return serde_json::from_slice(&bytes).ok();
        }
        sleep_ms(POLL_INTERVAL_MS);
        waited += POLL_INTERVAL_MS;
    }
    None
}

fn send_event(w: &OutputStream, value: &Value) {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(b"data: ");
    buf.extend_from_slice(&serde_json::to_vec(value).unwrap_or_default());
    buf.extend_from_slice(b"\n\n");
    write_all(w, &buf);
}

pub fn sleep_ms(ms: u64) {
    if ms == 0 {
        return;
    }
    let p = monotonic_clock::subscribe_duration(ms.saturating_mul(1_000_000));
    p.block();
}

/// Write a buffer in ≤4096-byte chunks, respecting the wasi:io write contract.
pub fn write_all(w: &OutputStream, mut data: &[u8]) {
    while !data.is_empty() {
        let n = data.len().min(4096);
        match w.blocking_write_and_flush(&data[..n]) {
            Ok(()) => data = &data[n..],
            Err(StreamError::Closed) => break,
            Err(StreamError::LastOperationFailed(_)) => break,
        }
    }
}
