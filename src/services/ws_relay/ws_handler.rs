use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info, warn};

use super::messages::GatewayMessage;
use super::registry::{ServerRegistry, WsSink};

/// Drive a single WebSocket connection to completion.
///
/// The first frame the client sends must be a `Register` message. After that
/// the handler loops over incoming frames and dispatches them. The connection
/// is removed from the registry when it closes for any reason.
pub async fn handle(stream: TcpStream, registry: ServerRegistry) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!(error = %e, "WebSocket handshake failed");
            return;
        }
    };

    let (sink, mut source) = ws.split();
    let sink: WsSink = Arc::new(Mutex::new(sink));

    // ── Wait for registration frame ──────────────────────────────────
    let server_url = loop {
        match source.next().await {
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<GatewayMessage>(&text) {
                    Ok(GatewayMessage::Register { server_url }) => break server_url,
                    Ok(other) => {
                        warn!(?other, "Expected Register frame, got something else");
                        let ack = GatewayMessage::Ack {
                            ok: false,
                            message: Some("first message must be Register".into()),
                        };
                        let _ = send(&sink, &ack).await;
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse registration frame");
                        let ack = GatewayMessage::Ack {
                            ok: false,
                            message: Some(format!("parse error: {e}")),
                        };
                        let _ = send(&sink, &ack).await;
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => return,
            Some(Ok(_)) => {} // ignore ping/binary/etc before registration
            Some(Err(e)) => {
                warn!(error = %e, "WebSocket error during registration");
                return;
            }
        }
    };

    registry.register(server_url.clone(), Arc::clone(&sink));
    let ack = GatewayMessage::Ack { ok: true, message: None };
    if let Err(e) = send(&sink, &ack).await {
        error!(server_url = %server_url, error = %e, "Failed to send registration ack");
        registry.deregister(&server_url);
        return;
    }
    info!(server_url = %server_url, "Registration acknowledged");

    // ── Main message loop ────────────────────────────────────────────
    while let Some(result) = source.next().await {
        match result {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<GatewayMessage>(&text) {
                    Ok(msg) => dispatch(msg, &server_url, &registry, &sink).await,
                    Err(e) => {
                        warn!(server_url = %server_url, error = %e, "Failed to parse frame");
                        let ack = GatewayMessage::Ack {
                            ok: false,
                            message: Some(format!("parse error: {e}")),
                        };
                        let _ = send(&sink, &ack).await;
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let mut s = sink.lock().await;
                let _ = s.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }

    registry.deregister(&server_url);
}

/// Route a parsed `GatewayMessage` from `from_server`.
async fn dispatch(
    msg: GatewayMessage,
    from_server: &str,
    registry: &ServerRegistry,
    sender_sink: &WsSink,
) {
    match msg {
        GatewayMessage::Register { .. } => {
            // Re-registration: silently ignore (already registered).
        }

        GatewayMessage::Connect { target } => {
            let request = GatewayMessage::ConnectRequest { from: from_server.to_string() };
            match registry.relay(&target, &request).await {
                Ok(()) => {
                    info!(from = %from_server, to = %target, "Relayed ConnectRequest");
                    let ack = GatewayMessage::Ack { ok: true, message: None };
                    let _ = send(sender_sink, &ack).await;
                }
                Err(e) => {
                    warn!(from = %from_server, to = %target, error = %e, "ConnectRequest failed");
                    let ack = GatewayMessage::Ack { ok: false, message: Some(e) };
                    let _ = send(sender_sink, &ack).await;
                }
            }
        }

        GatewayMessage::Relay { to, payload, .. } => {
            // Rewrite `from` to the authenticated server_url of this connection.
            let relay = GatewayMessage::Relay {
                from: from_server.to_string(),
                to: to.clone(),
                payload,
            };
            match registry.relay(&to, &relay).await {
                Ok(()) => {
                    info!(from = %from_server, to = %to, "Relayed frame");
                }
                Err(e) => {
                    warn!(from = %from_server, to = %to, error = %e, "Relay failed");
                    let ack = GatewayMessage::Ack { ok: false, message: Some(e) };
                    let _ = send(sender_sink, &ack).await;
                }
            }
        }

        GatewayMessage::ConnectRequest { .. }
        | GatewayMessage::Ack { .. } => {
            // These are gateway-to-client only; ignore if sent by a client.
        }
    }
}

async fn send(sink: &WsSink, msg: &GatewayMessage) -> Result<(), String> {
    let text = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    sink.lock()
        .await
        .send(Message::Text(text.into()))
        .await
        .map_err(|e| e.to_string())
}
