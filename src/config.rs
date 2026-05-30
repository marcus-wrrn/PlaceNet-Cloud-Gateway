//! Gateway configuration, loaded from environment variables.

/// All runtime configuration for the cloud gateway.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    // ── HTTP login API ──
    /// Bind address for the HTTPS `/api/login` API.
    pub api_host: String,
    pub api_port: u16,
    /// When true the API is served over TLS using `tls_cert`/`tls_key`.
    pub tls_enabled: bool,
    pub tls_cert: String,
    pub tls_key: String,

    // ── Credential store ──
    pub database_url: String,

    // ── dynsec admin connection (gateway → local broker) ──
    /// Host/port the gateway itself uses to reach the broker for dynsec control.
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub dynsec_admin_user: String,
    pub dynsec_admin_password: String,

    // ── Broker coordinates handed back to Hamlets in the login response ──
    /// Public hostname Hamlets should use to reach the MQTTS broker.
    pub broker_public_host: String,
    /// Public MQTTS port Hamlets should connect to.
    pub broker_public_port: u16,

    // ── Legacy WebSocket relay ──
    pub ws_port: u16,
}

impl GatewayConfig {
    pub fn from_env() -> Self {
        let api_host = std::env::var("GATEWAY_API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = parse_port("GATEWAY_API_PORT", 8443);
        let tls_enabled = parse_bool("GATEWAY_TLS_ENABLED", false);
        let tls_cert = std::env::var("GATEWAY_TLS_CERT").unwrap_or_else(|_| "certs/gateway.crt".to_string());
        let tls_key = std::env::var("GATEWAY_TLS_KEY").unwrap_or_else(|_| "certs/gateway.key".to_string());

        let database_url = std::env::var("GATEWAY_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://placenet_gateway.db".to_string());

        let mqtt_host = std::env::var("GATEWAY_MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
        let mqtt_port = parse_port("GATEWAY_MQTT_PORT", 1883);
        let dynsec_admin_user =
            std::env::var("DYNSEC_ADMIN_USER").unwrap_or_else(|_| "gateway-admin".to_string());
        let dynsec_admin_password =
            std::env::var("DYNSEC_ADMIN_PASSWORD").unwrap_or_else(|_| "changeme-admin".to_string());

        let broker_public_host = std::env::var("GATEWAY_BROKER_PUBLIC_HOST")
            .unwrap_or_else(|_| mqtt_host.clone());
        let broker_public_port = parse_port("GATEWAY_BROKER_PUBLIC_PORT", 8883);

        let ws_port = parse_port("GATEWAY_PORT", 8080);

        Self {
            api_host,
            api_port,
            tls_enabled,
            tls_cert,
            tls_key,
            database_url,
            mqtt_host,
            mqtt_port,
            dynsec_admin_user,
            dynsec_admin_password,
            broker_public_host,
            broker_public_port,
            ws_port,
        }
    }
}

fn parse_port(var: &str, default: u16) -> u16 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn parse_bool(var: &str, default: bool) -> bool {
    std::env::var(var)
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}
