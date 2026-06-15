use std::sync::Arc;

use dashmap::DashMap;
use futures_util::SinkExt;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::messages::GatewayMessage;

/// A thread-safe sender half of a WebSocket connection.
pub type WsSink = Arc<Mutex<futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>>>;

/// Shared registry of connected placenet-home servers.
///
/// Keys are the `server_url` strings each server provides during registration.
/// Values are the write halves of their WebSocket connections.
#[derive(Clone, Default)]
pub struct ServerRegistry {
    inner: Arc<DashMap<String, WsSink>>,
}

impl ServerRegistry {
    pub fn new() -> Self {
        Self { inner: Arc::new(DashMap::new()) }
    }

    /// Register a server under `server_url`.
    pub fn register(&self, server_url: String, sink: WsSink) {
        info!(server_url = %server_url, "Server registered");
        self.inner.insert(server_url, sink);
    }

    /// Remove a server from the registry.
    pub fn deregister(&self, server_url: &str) {
        if self.inner.remove(server_url).is_some() {
            info!(server_url = %server_url, "Server deregistered");
        }
    }

    /// Send `msg` to the server identified by `server_url`.
    ///
    /// Returns `Ok(())` if the frame was enqueued into the WebSocket sink.
    /// Returns `Err` if the target is not registered or the send fails.
    pub async fn relay(&self, server_url: &str, msg: &GatewayMessage) -> Result<(), String> {
        let sink = self
            .inner
            .get(server_url)
            .ok_or_else(|| format!("server '{}' not registered", server_url))?
            .clone();

        let text = serde_json::to_string(msg)
            .map_err(|e| format!("serialisation error: {e}"))?;

        sink.lock()
            .await
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| {
                warn!(server_url = %server_url, error = %e, "WebSocket send failed");
                format!("send error: {e}")
            })
    }

    /// Returns a snapshot of all currently registered server URLs.
    #[allow(dead_code)]
    pub fn list(&self) -> Vec<String> {
        self.inner.iter().map(|e| e.key().clone()).collect()
    }
}
