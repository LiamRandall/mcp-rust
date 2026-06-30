//! Shared WASI 0.2 glue for API-bridging MCP tool servers.
//!
//! An example component is a few `#[tool]` files plus one [`serve!`] call. This
//! crate owns everything wasm: the HTTP entrypoint, reading the request body,
//! writing the JSON response, and the outbound-HTTP/config backend that tool
//! bodies use through `mcp_core::http`.

#![allow(clippy::all)]

pub mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "api-server",
        generate_all,
        pub_export_macro: true,
    });
}

pub use bindings::wasi::http::types::{IncomingRequest, ResponseOutparam};
pub use mcp_api_core::ServerInfo;
pub use mcp_core::ToolHandle;

use bindings::wasi::http::types::{
    Fields, IncomingBody, Method, OutgoingBody, OutgoingRequest, OutgoingResponse, Scheme,
};
use bindings::wasi::io::streams::StreamError;

/// Entry point invoked by [`serve!`]. Installs the outbound backend, routes the
/// request through `mcp-api-core`, and writes the response.
pub fn serve(
    request: IncomingRequest,
    response_out: ResponseOutparam,
    info: ServerInfo,
    tools: Vec<ToolHandle>,
) {
    mcp_core::http::set_backend(Box::new(WasiBackend));

    match request.method() {
        Method::Post => {
            let body = read_body(request);
            let resp = mcp_api_core::handle(&body, &info, &tools);
            write_response(response_out, resp.status, resp.content_type, &resp.body);
        }
        // No standalone SSE stream (405 is tolerated by MCP clients); accept DELETE.
        Method::Get => write_response(response_out, 405, "application/json", &[]),
        Method::Delete => write_response(response_out, 200, "application/json", &[]),
        _ => write_response(response_out, 405, "application/json", &[]),
    }
}

// ---- request/response plumbing -----------------------------------------

fn read_body(request: IncomingRequest) -> Vec<u8> {
    let Ok(incoming) = request.consume() else { return Vec::new() };
    let Ok(stream) = incoming.stream() else { return Vec::new() };
    let mut buf = Vec::new();
    loop {
        match stream.blocking_read(65536) {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => buf.extend_from_slice(&chunk),
            Err(_) => break,
        }
    }
    drop(stream);
    IncomingBody::finish(incoming);
    buf
}

fn write_response(out: ResponseOutparam, status: u16, content_type: &str, body: &[u8]) {
    let headers = Fields::new();
    let _ = headers.append(&"content-type".to_string(), content_type.as_bytes());
    let resp = OutgoingResponse::new(headers);
    let _ = resp.set_status_code(status);
    let body_handle = resp.body().expect("outgoing body");
    ResponseOutparam::set(out, Ok(resp));
    {
        let stream = body_handle.write().expect("outgoing stream");
        write_all(&stream, body);
        drop(stream);
    }
    let _ = OutgoingBody::finish(body_handle, None);
}

fn write_all(stream: &bindings::wasi::io::streams::OutputStream, mut data: &[u8]) {
    while !data.is_empty() {
        let n = data.len().min(4096);
        match stream.blocking_write_and_flush(&data[..n]) {
            Ok(()) => data = &data[n..],
            Err(_) => break,
        }
    }
}

// ---- outbound HTTP + config backend ------------------------------------

struct WasiBackend;

impl mcp_core::http::Backend for WasiBackend {
    fn config(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn fetch(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
        headers: &[(String, String)],
    ) -> Result<mcp_core::http::Response, String> {
        let (scheme, authority, path) = split_url(url)?;

        let fields = Fields::new();
        for (k, v) in headers {
            let _ = fields.append(&k.to_ascii_lowercase(), v.as_bytes());
        }
        // Bearer token auto-attached from config if present (DESIGN §6).
        if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("authorization")) {
            if let Ok(token) = std::env::var("API_TOKEN") {
                let _ = fields.append(&"authorization".to_string(), format!("Bearer {token}").as_bytes());
            }
        }

        let req = OutgoingRequest::new(fields);
        req.set_method(&parse_method(method)).map_err(|_| "bad method".to_string())?;
        req.set_scheme(Some(&scheme)).map_err(|_| "bad scheme".to_string())?;
        req.set_authority(Some(&authority)).map_err(|_| "bad authority".to_string())?;
        req.set_path_with_query(Some(&path)).map_err(|_| "bad path".to_string())?;

        let out_body = req.body().map_err(|_| "no body handle".to_string())?;
        let future = bindings::wasi::http::outgoing_handler::handle(req, None)
            .map_err(|e| format!("send failed: {e:?}"))?;

        if !body.is_empty() {
            let stream = out_body.write().map_err(|_| "no body stream".to_string())?;
            write_all(&stream, body);
            drop(stream);
        }
        OutgoingBody::finish(out_body, None).map_err(|e| format!("finish body: {e:?}"))?;

        let pollable = future.subscribe();
        pollable.block();
        let response = future
            .get()
            .ok_or_else(|| "no response".to_string())?
            .map_err(|_| "response taken".to_string())?
            .map_err(|e| format!("http error: {e:?}"))?;

        let status = response.status();
        let incoming = response.consume().map_err(|_| "consume failed".to_string())?;
        let stream = incoming.stream().map_err(|_| "no response stream".to_string())?;
        let mut buf = Vec::new();
        loop {
            match stream.blocking_read(65536) {
                Ok(chunk) if chunk.is_empty() => break,
                Ok(chunk) => buf.extend_from_slice(&chunk),
                Err(StreamError::Closed) => break,
                Err(StreamError::LastOperationFailed(_)) => break,
            }
        }
        drop(stream);
        IncomingBody::finish(incoming);

        Ok(mcp_core::http::Response { status, body: buf })
    }
}

fn parse_method(method: &str) -> Method {
    match method.to_ascii_uppercase().as_str() {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        "PATCH" => Method::Patch,
        "HEAD" => Method::Head,
        other => Method::Other(other.to_string()),
    }
}

/// Split `scheme://authority/path?query` into the parts wasi:http wants.
fn split_url(url: &str) -> Result<(Scheme, String, String), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("invalid url: {url}"))?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "http" => Scheme::Http,
        "https" => Scheme::Https,
        other => Scheme::Other(other.to_string()),
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.to_string(), "/".to_string()),
    };
    Ok((scheme, authority, path))
}

/// Define the component for an API-bridging MCP server.
///
/// ```ignore
/// mcp_api_server::serve! {
///     name: "hello-world",
///     version: "0.1.0",
///     instructions: None,
///     tools: || vec![greet::greet_tool()],
/// }
/// ```
#[macro_export]
macro_rules! serve {
    (name: $name:expr, version: $version:expr, instructions: $instructions:expr, tools: $tools:expr $(,)?) => {
        struct __ApiComponent;

        impl $crate::bindings::exports::wasi::http::incoming_handler::Guest for __ApiComponent {
            fn handle(
                request: $crate::IncomingRequest,
                response_out: $crate::ResponseOutparam,
            ) {
                let make: fn() -> ::std::vec::Vec<$crate::ToolHandle> = $tools;
                $crate::serve(
                    request,
                    response_out,
                    $crate::ServerInfo {
                        name: $name,
                        version: $version,
                        instructions: $instructions,
                    },
                    make(),
                );
            }
        }

        $crate::bindings::export!(__ApiComponent with_types_in $crate::bindings);
    };
}
