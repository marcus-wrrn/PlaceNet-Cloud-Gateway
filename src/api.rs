//! HTTP login API (axum).
//!
//! `POST /api/login` validates a Hamlet's credentials, assigns/looks up its
//! device, generates a fresh MQTT password, provisions it in the broker via
//! dynsec, and returns the connection details. Returns 503 until the dynsec
//! admin link is live so a cold-starting Hamlet simply retries.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::Rng;
use serde_json::json;
use tracing::{error, info, warn};

use crate::config::GatewayConfig;
use crate::db::Store;
use crate::services::dynsec::DynsecHandle;
use crate::protocol::{BrokerInfo, DeviceTopics, LoginRequest, LoginResponse, PROTOCOL_VERSION};

#[derive(Clone)]
pub struct ApiState {
    pub store: Store,
    pub dynsec: DynsecHandle,
    pub config: Arc<GatewayConfig>,
    /// Set once the dynsec admin connection is live.
    pub ready: Arc<AtomicBool>,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/login", post(login))
        .route("/healthz", get(healthz))
        .with_state(state)
}

async fn healthz(State(state): State<ApiState>) -> Response {
    if state.ready.load(Ordering::Relaxed) {
        (StatusCode::OK, "ok").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "broker not ready").into_response()
    }
}

async fn login(State(state): State<ApiState>, Json(req): Json<LoginRequest>) -> Response {
    info!("Login Request received!");
    if !state.ready.load(Ordering::Relaxed) {
        warn!("login attempted before dynsec admin ready");
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "broker not ready, retry");
    }

    match state.store.verify_credential(&req.username, &req.password).await {
        Ok(true) => {}
        Ok(false) => {
            warn!(username = %req.username, "login rejected: bad credentials");
            return error_response(StatusCode::UNAUTHORIZED, "invalid credentials");
        }
        Err(e) => {
            error!("credential check failed: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    }

    let device = match state.store.get_or_create_device(&req.username).await {
        Ok(d) => d,
        Err(e) => {
            error!("device lookup failed: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };

    // 3. Generate a fresh MQTT password and provision it in the broker.
    let mqtt_password = generate_password();
    if let Err(e) = state
        .dynsec
        .provision_device(&device.device_id, &mqtt_password)
        .await
    {
        error!(device_id = %device.device_id, "dynsec provisioning failed: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "provisioning failed");
    }

    info!(username = %req.username, device_id = %device.device_id, "login ok");

    // 4. Return the connection contract.
    let resp = LoginResponse {
        protocol_version: PROTOCOL_VERSION.to_string(),
        device_id: device.device_id.clone(),
        mqtt_username: device.mqtt_username,
        mqtt_password,
        broker: BrokerInfo {
            host: state.config.broker_public_host.clone(),
            port: state.config.broker_public_port,
        },
        topics: DeviceTopics::for_device(&device.device_id),
    };
    (StatusCode::OK, Json(resp)).into_response()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// 24 random bytes rendered as lowercase hex (48 chars) — a broker password.
fn generate_password() -> String {
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
