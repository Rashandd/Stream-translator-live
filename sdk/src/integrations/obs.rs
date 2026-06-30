//! OBS Studio integration via obs-websocket v5.
//!
//! Used to push live subtitles into an OBS text source (overlay). This module
//! builds the v5 protocol JSON payloads; the actual socket connection + the
//! op-1/op-2 Identify handshake are wired when the SDK runtime lands (requires a
//! running OBS with obs-websocket enabled, so it is integration-tested manually).
//!
//! Reference: obs-websocket 5.x message ops (Hello=0, Identify=1, Request=6).

use serde::Serialize;
use serde_json::{json, Value};

/// obs-websocket v5 opcodes (subset used here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Identify = 1,
    Request = 6,
}

/// Builds an `Identify` (op 1) message. `rpc_version` is 1 for v5.
pub fn identify(rpc_version: u32, auth: Option<&str>) -> Value {
    let mut data = json!({ "rpcVersion": rpc_version });
    if let Some(auth) = auth {
        data["authentication"] = json!(auth);
    }
    json!({ "op": OpCode::Identify as u32, "d": data })
}

/// Parameters for updating a text input's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetTextRequest {
    /// OBS text input (source) name, e.g. "StreamDubber Subtitles".
    pub input_name: String,
    /// New subtitle text to display.
    pub text: String,
}

impl SetTextRequest {
    pub fn new(input_name: impl Into<String>, text: impl Into<String>) -> Self {
        SetTextRequest {
            input_name: input_name.into(),
            text: text.into(),
        }
    }

    /// Builds the obs-websocket `Request` (op 6) `SetInputSettings` payload.
    ///
    /// `request_id` correlates the response. The text field name `text` matches
    /// OBS Free Type / GDI text sources.
    pub fn to_request(&self, request_id: &str) -> Value {
        json!({
            "op": OpCode::Request as u32,
            "d": {
                "requestType": "SetInputSettings",
                "requestId": request_id,
                "requestData": {
                    "inputName": self.input_name,
                    "inputSettings": { "text": self.text },
                    "overlay": true
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_has_op_1_and_rpc_version() {
        let msg = identify(1, None);
        assert_eq!(msg["op"], 1);
        assert_eq!(msg["d"]["rpcVersion"], 1);
        assert!(msg["d"].get("authentication").is_none());
    }

    #[test]
    fn identify_includes_auth_when_present() {
        let msg = identify(1, Some("hash"));
        assert_eq!(msg["d"]["authentication"], "hash");
    }

    #[test]
    fn set_text_builds_set_input_settings_request() {
        let req = SetTextRequest::new("Subtitles", "merhaba dünya");
        let payload = req.to_request("req-1");
        assert_eq!(payload["op"], 6);
        assert_eq!(payload["d"]["requestType"], "SetInputSettings");
        assert_eq!(payload["d"]["requestId"], "req-1");
        assert_eq!(payload["d"]["requestData"]["inputName"], "Subtitles");
        assert_eq!(
            payload["d"]["requestData"]["inputSettings"]["text"],
            "merhaba dünya"
        );
    }
}
