mod messages;
mod registry;
mod ws_handler;

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tracing::info;

use registry::ServerRegistry;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let port: u16 = std::env::var("GATEWAY_PORT")
        .unwrap_or_else(|_| "9000".to_string())
        .parse()
        .unwrap_or(9000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("Failed to bind TCP listener");
    let registry = ServerRegistry::new();

    info!("PlaceNet cloud gateway listening on ws://{addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                info!(peer = %peer_addr, "Incoming connection");
                let registry = registry.clone();
                tokio::spawn(async move {
                    ws_handler::handle(stream, registry).await;
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "Accept error");
            }
        }
    }
}
