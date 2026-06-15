//! Legacy WebSocket relay (kept until the MQTT router replaces it).

pub mod messages;
pub mod registry;
pub mod ws_handler;

pub use registry::ServerRegistry;
pub use ws_handler::handle;
