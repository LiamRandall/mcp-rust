//! Cross-request correlation via the built-in `wasi:keyvalue` host plugin.
//!
//! Server→client requests (sampling / elicitation) are issued on one HTTP
//! request's SSE stream, but the client delivers its response on a *separate*
//! POST — a different component instance. The shared keyvalue store is how the
//! waiting instance observes the response. Under `wasmtime serve -S keyvalue`
//! and the wasmCloud keyvalue plugin alike, the store is process-shared.

use crate::wasi::keyvalue::store;

fn bucket() -> Option<store::Bucket> {
    store::open("").ok()
}

pub fn put(key: &str, value: &[u8]) {
    if let Some(b) = bucket() {
        let _ = b.set(key, value);
    }
}

pub fn take(key: &str) -> Option<Vec<u8>> {
    let b = bucket()?;
    let v = b.get(key).ok().flatten();
    if v.is_some() {
        let _ = b.delete(key);
    }
    v
}

/// A unique id for a server-initiated request, derived from host randomness.
pub fn unique_id() -> String {
    let bytes = crate::wasi::random::random::get_random_bytes(12);
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("s-");
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}
