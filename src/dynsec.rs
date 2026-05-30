//! Mosquitto Dynamic Security admin client.
//!
//! The gateway connects to its local broker as the dynsec admin user and issues
//! commands on `$CONTROL/dynamic-security/v1`, reading correlated responses on
//! `$CONTROL/dynamic-security/v1/response`. It provisions one role + one client
//! per device, with ACLs scoped to exactly that device's three topics.
//!
//! All provisioning is **create-or-update / idempotent**: "already exists"
//! responses are treated as success, and the client password is (re)set on
//! every login so the just-issued credential always works.

use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

const CONTROL_TOPIC: &str = "$CONTROL/dynamic-security/v1";
const RESPONSE_TOPIC: &str = "$CONTROL/dynamic-security/v1/response";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A request handed to the dynsec actor: a batch of commands plus a reply slot
/// that receives the parsed `responses` array (or an error string).
struct DynsecRequest {
    commands: Vec<Value>,
    reply: oneshot::Sender<Result<Vec<Value>, String>>,
}

/// Cloneable handle for provisioning devices in the broker.
#[derive(Clone)]
pub struct DynsecHandle {
    tx: mpsc::Sender<DynsecRequest>,
}

impl DynsecHandle {
    /// Provision (create-or-update) a device's MQTT client, role, and ACLs, and
    /// set its password to `password`. Idempotent across repeated logins.
    pub async fn provision_device(&self, device_id: &str, password: &str) -> Result<(), String> {
        let role = role_name(device_id);
        let cmds = device_id;
        let connect = format!("placenet/{device_id}/connect");
        let notify = format!("placenet/{device_id}/notify");
        let cmds_topic = format!("placenet/{device_id}/cmds");

        // Role with ACLs scoped to exactly this device's three topics.
        let create_role = json!({ "command": "createRole", "rolename": role });
        let acls = [
            // device may publish to its own cmds + notify
            acl_cmd(&role, "publishClientSend", &cmds_topic),
            acl_cmd(&role, "publishClientSend", &notify),
            // device may receive on its own cmds + connect
            acl_cmd(&role, "publishClientReceive", &cmds_topic),
            acl_cmd(&role, "publishClientReceive", &connect),
            // device may subscribe to its own cmds + connect
            acl_cmd(&role, "subscribePattern", &cmds_topic),
            acl_cmd(&role, "subscribePattern", &connect),
        ];

        let create_client = json!({
            "command": "createClient",
            "username": cmds,
            "password": password,
            "roles": [ { "rolename": role, "priority": -1 } ],
        });
        let set_password = json!({
            "command": "setClientPassword",
            "username": cmds,
            "password": password,
        });

        let mut batch = vec![create_role];
        batch.extend(acls);
        batch.push(create_client);
        batch.push(set_password);

        let responses = self.send(batch).await?;
        check_responses(&responses)?;
        info!(device_id, "dynsec device provisioned");
        Ok(())
    }

    async fn send(&self, commands: Vec<Value>) -> Result<Vec<Value>, String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(DynsecRequest { commands, reply })
            .await
            .map_err(|_| "dynsec actor unavailable".to_string())?;
        rx.await.map_err(|_| "dynsec actor dropped reply".to_string())?
    }
}

fn role_name(device_id: &str) -> String {
    format!("device-{device_id}")
}

fn acl_cmd(role: &str, acltype: &str, topic: &str) -> Value {
    json!({
        "command": "addRoleACL",
        "rolename": role,
        "acltype": acltype,
        "topic": topic,
        "priority": 0,
        "allow": true,
    })
}

/// Treat each response as success unless it carries an error that is not a
/// benign "already exists". dynsec returns errors as a string `error` field.
fn check_responses(responses: &[Value]) -> Result<(), String> {
    for resp in responses {
        if let Some(err) = resp.get("error").and_then(Value::as_str) {
            let lower = err.to_ascii_lowercase();
            if lower.contains("already exists") {
                continue;
            }
            let command = resp.get("command").and_then(Value::as_str).unwrap_or("?");
            return Err(format!("dynsec '{command}' failed: {err}"));
        }
    }
    Ok(())
}

/// Spawn the dynsec admin client. Returns a handle and a `oneshot` that fires
/// once the admin connection is established (used to gate API readiness).
pub fn spawn(
    host: String,
    port: u16,
    admin_user: String,
    admin_password: String,
) -> (DynsecHandle, oneshot::Receiver<()>) {
    let (tx, rx) = mpsc::channel::<DynsecRequest>(32);
    let (ready_tx, ready_rx) = oneshot::channel::<()>();
    tokio::spawn(run_actor(host, port, admin_user, admin_password, rx, ready_tx));
    (DynsecHandle { tx }, ready_rx)
}

async fn run_actor(
    host: String,
    port: u16,
    admin_user: String,
    admin_password: String,
    mut rx: mpsc::Receiver<DynsecRequest>,
    ready_tx: oneshot::Sender<()>,
) {
    let mut opts = MqttOptions::new("gateway-dynsec-admin", &host, port);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_credentials(&admin_user, &admin_password);

    let (client, mut eventloop) = AsyncClient::new(opts, 32);

    // Pending request awaiting a response (dynsec processes one batch at a time).
    let mut pending: Option<oneshot::Sender<Result<Vec<Value>, String>>> = None;
    let mut ready_tx = Some(ready_tx);
    let mut subscribed = false;

    loop {
        tokio::select! {
            // New provisioning request — only accept when idle and connected.
            maybe_req = rx.recv(), if pending.is_none() && subscribed => {
                let Some(req) = maybe_req else { break };
                let payload = json!({ "commands": req.commands }).to_string();
                match client.publish(CONTROL_TOPIC, QoS::AtLeastOnce, false, payload).await {
                    Ok(()) => pending = Some(req.reply),
                    Err(e) => { let _ = req.reply.send(Err(format!("dynsec publish failed: {e}"))); }
                }
            }

            poll = eventloop.poll() => {
                match poll {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        match client.subscribe(RESPONSE_TOPIC, QoS::AtLeastOnce).await {
                            Ok(()) => {
                                subscribed = true;
                                info!("dynsec admin connected and subscribed");
                                if let Some(tx) = ready_tx.take() { let _ = tx.send(()); }
                            }
                            Err(e) => error!("dynsec response subscribe failed: {e}"),
                        }
                    }
                    Ok(Event::Incoming(Packet::Publish(p))) if p.topic == RESPONSE_TOPIC => {
                        if let Some(reply) = pending.take() {
                            let _ = reply.send(parse_response(&p.payload));
                        }
                    }
                    Ok(Event::Incoming(Packet::Disconnect)) => {
                        warn!("dynsec admin disconnected");
                        subscribed = false;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("dynsec eventloop error: {e}");
                        subscribed = false;
                        // Fail any in-flight request so the caller doesn't hang.
                        if let Some(reply) = pending.take() {
                            let _ = reply.send(Err(format!("dynsec connection error: {e}")));
                        }
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            // Safety net: time out a request that never gets a response.
            _ = tokio::time::sleep(REQUEST_TIMEOUT), if pending.is_some() => {
                if let Some(reply) = pending.take() {
                    let _ = reply.send(Err("dynsec request timed out".to_string()));
                }
            }
        }
    }
}

fn parse_response(payload: &[u8]) -> Result<Vec<Value>, String> {
    let value: Value =
        serde_json::from_slice(payload).map_err(|e| format!("invalid dynsec response: {e}"))?;
    value
        .get("responses")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "dynsec response missing 'responses' array".to_string())
}
