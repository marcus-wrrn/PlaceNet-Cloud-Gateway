mod api;
mod config;
mod db;
mod protocol;
mod services;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};
use tracing::{error, info};

use api::ApiState;
use config::GatewayConfig;
use db::Store;

#[derive(Parser)]
#[command(name = "placenet-cloud-gateway", about = "PlaceNet cloud gateway")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gateway (login API, dynsec admin). Default.
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

    // ── Embedded Mosquitto broker (spawned + supervised by this process) ──
    if let Err(e) = services::mqtt_brokerage::start_supervised(config.as_ref()).await {
        error!("failed to start mosquitto broker: {e}");
        std::process::exit(1);
    }

    // ── dynsec admin client ──
    let (dynsec_handle, ready_rx) = services::dynsec::spawn(
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

    // TLS is terminated by nginx; serve plain HTTP behind the reverse proxy.
    info!("login API listening on http://{addr}");
    if let Err(e) = axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
    {
        error!("login API server error: {e}");
    }
}
