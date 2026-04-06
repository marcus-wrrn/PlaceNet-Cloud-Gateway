use serde::{Deserialize, Serialize};

/// All frames exchanged over the gateway WebSocket connection.
///
/// Each variant is tagged with a `"type"` field in JSON so the receiving side
/// can dispatch without needing a wrapper envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayMessage {
    /// Sent by a placenet-home server immediately after connecting.
    /// Registers the server in the gateway's in-memory registry under
    /// `server_url`, which is used as its stable identity/ID.
    Register { server_url: String },

    /// Sent by server A to request a relay session with server B.
    /// `target` must match the `server_url` B used when registering.
    Connect { target: String },

    /// Forwarded by the gateway to server B when server A requests a session.
    ConnectRequest { from: String },

    /// A relay frame. The gateway rewrites `from` to the authenticated
    /// `server_url` of the sending connection before forwarding.
    Relay {
        from: String,
        to: String,
        payload: serde_json::Value,
    },

    /// Generic acknowledgement / error response from the gateway.
    Ack {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}
