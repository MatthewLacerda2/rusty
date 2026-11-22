//! Empty without its own `dev` feature: `cargo build --workspace` builds every member,
//! so the crate gates itself, not only the root's optional dependency on it.
#![cfg(feature = "dev")]
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "platform layer, exempt from the sim's clock ban (#757): frame timing and screenshot timeouts measure real time"
)]

pub mod bake;
pub mod bench;
pub mod botplayer;
pub mod bridge;
pub mod capture;
pub mod command_channel;
pub mod console;
pub mod demo;
pub mod export;
pub mod frame_log;
pub mod game;
pub mod harness;
pub mod lua_surface;
pub mod mcp;
pub mod memory;
pub mod preview;
pub mod scenario;
pub mod screenshot;
pub mod session;
pub mod snapshot;
pub mod stats;
