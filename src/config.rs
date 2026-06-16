//! Gateway configuration, loaded from environment variables.

use std::path::PathBuf;

/// All runtime configuration for the cloud gateway.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    // ── HTTP login API (TLS terminated by nginx) ──
    /// Bind address for the `/api/login` API.
    pub api_host: String,
    pub api_port: u16,

    // ── Credential store ──
    pub database_url: String,

    // ── dynsec admin connection (gateway → local broker) ──
    /// Host/port the gateway itself uses to reach the broker for dynsec control.
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub dynsec_admin_user: String,
    pub dynsec_admin_password: String,

    // ── Embedded Mosquitto broker (spawned + supervised by the gateway) ──
    /// `mosquitto` broker binary (name on PATH or absolute path).
    pub mosquitto_binary: String,
    /// `mosquitto_ctrl` binary, used to bootstrap the dynsec state offline.
    pub mosquitto_ctrl_binary: String,
    /// Path to the dynamic-security plugin shared object.
    pub dynsec_plugin: String,
    /// Directory for the generated `mosquitto.conf`.
    pub config_dir: PathBuf,
    /// Directory for broker persistence + the dynsec state file.
    pub data_dir: PathBuf,
    /// Broker TLS cert/key for the public MQTTS listener.
    pub broker_certfile: PathBuf,
    pub broker_keyfile: PathBuf,
    /// Port the broker's public MQTTS listener binds to.
    pub broker_mqtts_port: u16,

    // ── Broker coordinates handed back to Hamlets in the login response ──
    /// Public hostname Hamlets should use to reach the MQTTS broker.
    pub broker_public_host: String,
    /// Public MQTTS port Hamlets should connect to.
    pub broker_public_port: u16,
}

impl GatewayConfig {
    pub fn from_env() -> Self {
        let api_host = std::env::var("GATEWAY_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = parse_port("GATEWAY_API_PORT", 8443);

        let database_url = std::env::var("GATEWAY_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://placenet_gateway.db".to_string());

        let mqtt_host = std::env::var("GATEWAY_MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
        let mqtt_port = parse_port("GATEWAY_MQTT_PORT", 1883);
        let dynsec_admin_user =
            std::env::var("DYNSEC_ADMIN_USER").unwrap_or_else(|_| "gateway-admin".to_string());
        let dynsec_admin_password =
            std::env::var("DYNSEC_ADMIN_PASSWORD").unwrap_or_else(|_| "changeme-admin".to_string());

        let mosquitto_binary =
            std::env::var("MOSQUITTO_BINARY").unwrap_or_else(|_| "mosquitto".to_string());
        let mosquitto_ctrl_binary =
            std::env::var("MOSQUITTO_CTRL_BINARY").unwrap_or_else(|_| "mosquitto_ctrl".to_string());
        let dynsec_plugin = std::env::var("MOSQUITTO_DYNSEC_PLUGIN")
            .unwrap_or_else(|_| "/usr/lib/x86_64-linux-gnu/mosquitto_dynamic_security.so".to_string());
        let config_dir =
            PathBuf::from(std::env::var("GATEWAY_CONFIG_DIR").unwrap_or_else(|_| "config".to_string()));
        let data_dir =
            PathBuf::from(std::env::var("GATEWAY_DATA_DIR").unwrap_or_else(|_| "data".to_string()));
        let broker_certfile = PathBuf::from(
            std::env::var("GATEWAY_BROKER_CERT")
                .unwrap_or_else(|_| "certs/broker/server.crt".to_string()),
        );
        let broker_keyfile = PathBuf::from(
            std::env::var("GATEWAY_BROKER_KEY")
                .unwrap_or_else(|_| "certs/broker/server.key".to_string()),
        );
        let broker_mqtts_port = parse_port("GATEWAY_BROKER_MQTTS_PORT", 8883);

        let broker_public_host = std::env::var("GATEWAY_BROKER_PUBLIC_HOST")
            .unwrap_or_else(|_| mqtt_host.clone());
        let broker_public_port = parse_port("GATEWAY_BROKER_PUBLIC_PORT", 8883);

        Self {
            api_host,
            api_port,
            database_url,
            mqtt_host,
            mqtt_port,
            dynsec_admin_user,
            dynsec_admin_password,
            mosquitto_binary,
            mosquitto_ctrl_binary,
            dynsec_plugin,
            config_dir,
            data_dir,
            broker_certfile,
            broker_keyfile,
            broker_mqtts_port,
            broker_public_host,
            broker_public_port,
        }
    }
}

fn parse_port(var: &str, default: u16) -> u16 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}
