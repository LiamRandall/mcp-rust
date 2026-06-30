//! Fixed MCP `2025-11-25` transport, compiled to a single WASI 0.2 component.
//!
//! All MCP/JSON-RPC framing, lifecycle, Streamable-HTTP transport, SSE, and the
//! server→client request orchestration (sampling / elicitation) live here, so a
//! tool author can never get protocol framing wrong. See DESIGN.md §3.

#![allow(clippy::all)]

wit_bindgen::generate!({
    path: "wit",
    world: "reference-server",
    generate_all,
});

mod fixtures;
mod kv;
mod proto;
mod sse;

use wasi::http::types::{
    Fields, IncomingBody, IncomingRequest, Method, OutgoingBody, OutgoingResponse,
    ResponseOutparam,
};
use wasi::io::streams::{InputStream, StreamError};

struct Component;

export!(Component);

impl exports::wasi::http::incoming_handler::Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let authority = request.authority();

        // DNS-rebinding protection (DESIGN; conformance `dns-rebinding-protection`).
        // MUST reject non-localhost Host/Origin for an unauthenticated localhost server.
        // wasmtime maps the HTTP `Host` header to the request authority, so check
        // both the authority and any `Origin` header. `headers` is a child of the
        // request resource and MUST be dropped before the request itself, so scope
        // it tightly and extract only owned values.
        let allowed = {
            let headers = request.headers();
            host_is_allowed(&authority, &headers)
        };
        if !allowed {
            respond_status(
                response_out,
                403,
                Some(b"{\"error\":\"forbidden: invalid Host/Origin\"}"),
            );
            return;
        }

        match method {
            Method::Post => handle_post(request, response_out),
            // Standalone SSE stream — we do not offer one; 405 is explicitly tolerated
            // by the MCP SDK client (streamableHttp.js: 405 => no GET SSE).
            Method::Get => respond_status(response_out, 405, None),
            // Session termination — accept gracefully.
            Method::Delete => respond_status(response_out, 200, None),
            _ => respond_status(response_out, 405, None),
        }
    }
}

fn handle_post(request: IncomingRequest, response_out: ResponseOutparam) {
    let body = match read_body(request) {
        Ok(b) => b,
        Err(_) => {
            respond_status(response_out, 400, Some(b"{\"error\":\"bad body\"}"));
            return;
        }
    };

    let msg: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            respond_json(
                response_out,
                &proto::error_envelope(serde_json::Value::Null, -32700, "Parse error"),
                None,
            );
            return;
        }
    };

    match proto::route(&msg) {
        proto::Reply::Json(value, session) => respond_json(response_out, &value, session),
        proto::Reply::Accepted => respond_status(response_out, 202, None),
        proto::Reply::Sse(plan) => sse::stream(response_out, plan),
    }
}

// ---- HTTP helpers -------------------------------------------------------

/// Localhost allow-list (DNS-rebinding protection). A missing Host/Origin is
/// allowed; a present one must resolve to localhost.
fn host_is_allowed(authority: &Option<String>, headers: &Fields) -> bool {
    let host_ok = |hostport: &str| -> bool {
        let hp = hostport.trim();
        let host = match hp.rfind(':') {
            // keep `[::1]:port` bracketed host intact
            Some(idx) if !hp[idx..].contains(']') => &hp[..idx],
            _ => hp,
        };
        let host = host.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
        host == "localhost" || host == "127.0.0.1" || host == "::1"
    };

    if let Some(a) = authority {
        if !host_ok(a) {
            return false;
        }
    }
    for v in headers.get(&"origin".to_string()) {
        let s = String::from_utf8_lossy(&v);
        let after_scheme = s.split("://").last().unwrap_or(&s);
        let hostport = after_scheme.split('/').next().unwrap_or(after_scheme);
        if !host_ok(hostport) {
            return false;
        }
    }
    true
}

fn read_body(request: IncomingRequest) -> Result<Vec<u8>, ()> {
    let incoming: IncomingBody = request.consume()?;
    let stream: InputStream = incoming.stream()?;
    let mut buf = Vec::new();
    loop {
        match stream.blocking_read(65536) {
            // `blocking_read` only returns once data is ready or the stream
            // closes; an empty chunk therefore signals end of body.
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => buf.extend_from_slice(&chunk),
            Err(StreamError::Closed) => break,
            Err(StreamError::LastOperationFailed(_)) => break,
        }
    }
    drop(stream);
    IncomingBody::finish(incoming);
    Ok(buf)
}

fn respond_json(out: ResponseOutparam, value: &serde_json::Value, session: Option<String>) {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let headers = Fields::new();
    let _ = headers.append(&"content-type".to_string(), b"application/json");
    if let Some(sid) = session {
        let _ = headers.append(&"mcp-session-id".to_string(), sid.as_bytes());
    }
    write_response(out, 200, headers, &bytes);
}

fn respond_status(out: ResponseOutparam, status: u16, body: Option<&[u8]>) {
    let headers = Fields::new();
    if body.is_some() {
        let _ = headers.append(&"content-type".to_string(), b"application/json");
    }
    write_response(out, status, headers, body.unwrap_or(&[]));
}

/// Build a complete (non-streaming) response and hand it to the host.
fn write_response(out: ResponseOutparam, status: u16, headers: Fields, body: &[u8]) {
    let resp = OutgoingResponse::new(headers);
    let _ = resp.set_status_code(status);
    let body_handle = resp.body().expect("outgoing body");
    ResponseOutparam::set(out, Ok(resp));
    {
        let stream = body_handle.write().expect("outgoing stream");
        sse::write_all(&stream, body);
        drop(stream);
    }
    let _ = OutgoingBody::finish(body_handle, None);
}
