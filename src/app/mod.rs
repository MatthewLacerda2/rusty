//! src/app/ — the application spine: a tickable GameWorld driven by a Schedule.
//!
//! The simulation is decoupled from winit/wgpu: it lives in `GameWorld::tick`,
//! which knows nothing about the window or the GPU. That decoupling is what makes
//! headless play possible.
//!
//! The per-frame tick is no longer a hand-wired call sequence. Modules self-register
//! their systems into ordered stages via `register(&mut App)` (see `app::app`), and
//! the resulting [`Schedule`] runs them each frame in
//! `FixedUpdate → Update → LateUpdate → Render` order, `Startup` once on Play. Order
//! within a stage is registration order. Storage is still the legacy `Scene`; the
//! hecs migration and the &mut World/Resources threading (issue #39) come later.

pub mod game;
mod particles;
mod play;
mod registry;
mod schedule;
mod stage;
mod system;

pub use game::{GameWorld, PlayTransition};
pub use registry::{build, App};
pub use schedule::Schedule;
pub use stage::Stage;
pub use system::System;
