mod api;
mod config;
mod db;
mod dynsec;
mod protocol;

// ── Legacy WebSocket relay (kept until the MQTT router replaces it) ──
mod messages;
mod registry;
mod ws_handler;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use api::ApiState;
use config::GatewayConfig;
use db::Store;
use registry::ServerRegistry;

#[derive(Parser)]
#[command(name = "placenet-cloud-gateway", about = "PlaceNet cloud gateway")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gateway (login API, dynsec admin, legacy WS relay). Default.
    Serve,
    /// Seed or update a Hamlet login credential in the store.
    SeedUser {
        username: String,
        /// Password (omit to be prompted interactively).
        #[arg(long)]
        password: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Install the rustls crypto provider once, before any TLS is built.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();
    let config = GatewayConfig::from_env();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config).await,
        Command::SeedUser { username, password } => seed_user(config, username, password).await,
    }
}

async fn seed_user(config: GatewayConfig, username: String, password: Option<String>) {
    let store = match Store::connect(&config.database_url).await {
        Ok(s) => s,
        Err(e) => {
            error!("failed to open store: {e}");
            std::process::exit(1);
        }
    };

    let password = password.unwrap_or_else(|| {
        rpassword::prompt_password(format!("Password for '{username}': "))
            .unwrap_or_else(|e| {
                error!("failed to read password: {e}");
                std::process::exit(1);
            })
    });

    if password.is_empty() {
        error!("password must not be empty");
        std::process::exit(1);
    }

    match store.upsert_credential(&username, &password).await {
        Ok(()) => info!(username = %username, "credential seeded"),
        Err(e) => {
            error!("failed to seed credential: {e}");
            std::process::exit(1);
        }
    }
}

async fn serve(config: GatewayConfig) {
    let config = Arc::new(config);

    // ── Credential store ──
    let store = match Store::connect(&config.database_url).await {
        Ok(s) => s,
        Err(e) => {
            error!("failed to open store: {e}");
            std::process::exit(1);
        }
    };

    // ── dynsec admin client ──
    let (dynsec_handle, ready_rx) = dynsec::spawn(
        config.mqtt_host.clone(),
        config.mqtt_port,
        config.dynsec_admin_user.clone(),
        config.dynsec_admin_password.clone(),
    );
    let ready = Arc::new(AtomicBool::new(false));
    {
        let ready = Arc::clone(&ready);
        tokio::spawn(async move {
            if ready_rx.await.is_ok() {
                ready.store(true, Ordering::Relaxed);
                info!("login API marked ready");
            }
        });
    }

    // ── Legacy WS relay (separate task/port) ──
    tokio::spawn(run_ws_relay(config.ws_port));

    // ── Login API ──
    let state = ApiState {
        store,
        dynsec: dynsec_handle,
        config: Arc::clone(&config),
        ready,
    };
    let app = api::router(state);
    let addr = SocketAddr::new(
        config.api_host.parse().unwrap_or_else(|_| [0, 0, 0, 0].into()),
        config.api_port,
    );

    if config.tls_enabled {
        info!("login API listening on https://{addr}");
        let tls = match axum_server::tls_rustls::RustlsConfig::from_pem_file(
            &config.tls_cert,
            &config.tls_key,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                error!("failed to load TLS cert/key: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = axum_server::bind_rustls(addr, tls)
            .serve(app.into_make_service())
            .await
        {
            error!("login API server error: {e}");
        }
    } else {
        warn!("GATEWAY_TLS_ENABLED=false — serving login API over plain HTTP (dev only)");
        info!("login API listening on http://{addr}");
        if let Err(e) = axum_server::bind(addr)
            .serve(app.into_make_service())
            .await
        {
            error!("login API server error: {e}");
        }
    }
}

/// Legacy WebSocket relay listener (unchanged behaviour, kept for the deferred
/// peer-coordination path until the MQTT router replaces it).
async fn run_ws_relay(port: u16) {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("failed to bind legacy WS listener on {addr}: {e}");
            return;
        }
    };
    let registry = ServerRegistry::new();
    info!("legacy WS relay listening on ws://{addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                info!(peer = %peer_addr, "Incoming WS connection");
                let registry = registry.clone();
                tokio::spawn(async move {
                    ws_handler::handle(stream, registry).await;
                });
            }
            Err(e) => warn!(error = %e, "WS accept error"),
        }
    }
}
