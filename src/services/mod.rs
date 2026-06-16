//! Background services owned by the gateway process.
//!
//! Each service lives in its own module with the placenet-home layout: a
//! `mod.rs` that re-exports the public surface and a `*_service.rs` holding the
//! implementation. The gateway has no central supervisor — services are spawned
//! directly from `main::serve()` — so there is no `ManagedService`/`Supervisor`
//! machinery here, just the per-service modules.

pub mod dynsec;
pub mod mqtt_brokerage;
