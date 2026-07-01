//! `Content-Length`-framed JSON-RPC message writing.
//!
//! `lsp_server::Message::write` applies this same framing but only for the
//! `Request`/`Response`/`Notification` shapes, and `lsp_server::Response`
//! cannot encode a JSON-RPC `"id": null` response (its `RequestId` has no
//! null variant). Spec ch. 16 §16.4 requires exactly that shape for a parse
//! error, so outgoing frames are written from plain `serde_json::Value`
//! envelopes here instead, reusing the identical wire framing.

use std::io::{self, Write};

pub fn write_frame(writer: &mut impl Write, value: &serde_json::Value) -> io::Result<()> {
    let body = serde_json::to_string(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body.as_bytes())?;
    writer.flush()
}

#[must_use]
pub fn ok_response(id: serde_json::Value, result: impl serde::Serialize) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

#[must_use]
pub fn error_response(id: serde_json::Value, code: i32, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

#[must_use]
pub fn notification(method: &str, params: impl serde::Serialize) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}
