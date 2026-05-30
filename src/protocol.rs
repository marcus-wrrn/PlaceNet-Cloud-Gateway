//! The MQTTS migration wire contract, shared in spirit with `placenet-home`.
//!
//! These types are the frozen Phase-0 contract: the `/api/login` request/
//! response, and the JSON envelope carried on the per-device topics. The
//! envelope is `#[serde(tag = "type")]` so new variants (`Connect`, `Relay`,
//! `Ack`) can be added later without breaking existing peers.

use serde::{Deserialize, Serialize};

/// Current registration protocol version. Sent in the login response so the
/// Hamlet can detect an incompatible gateway.
pub const PROTOCOL_VERSION: &str = "0.0.1";

/// `POST /api/login` request body.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Broker coordinates a Hamlet uses to open its MQTTS connection.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrokerInfo {
    pub host: String,
    pub port: u16,
}

/// The three per-device topics, fully qualified (`placenet/<device_id>/...`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceTopics {
    /// Subscribe + publish.
    pub cmds: String,
    /// Subscribe only.
    pub connect: String,
    /// Publish only.
    pub notify: String,
}

impl DeviceTopics {
    pub fn for_device(device_id: &str) -> Self {
        Self {
            cmds: format!("placenet/{device_id}/cmds"),
            connect: format!("placenet/{device_id}/connect"),
            notify: format!("placenet/{device_id}/notify"),
        }
    }
}

/// `POST /api/login` success response body.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginResponse {
    pub protocol_version: String,
    pub device_id: String,
    pub mqtt_username: String,
    pub mqtt_password: String,
    pub broker: BrokerInfo,
    pub topics: DeviceTopics,
}

/// JSON envelope carried on the `cmds` / `connect` / `notify` topics.
///
/// Only `Alive` is exercised this iteration; the remaining coordination
/// variants are reserved for the deferred server-side router.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)] // contract type; gateway-side consumer arrives with the deferred router
pub enum Envelope {
    /// Published by a Hamlet on its `notify` topic once connected.
    Alive { device_id: String },
    // Reserved for the deferred relay/coordination router:
    // Connect { target: String },
    // ConnectRequest { from: String },
    // Relay { from: String, to: String, payload: serde_json::Value },
    // Ack { ok: bool, message: Option<String> },
}
