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
//! within a stage is registration order.
//!
//! A system is the canonical two-argument `fn(&mut World, &mut Resources)` (issue
//! #39): [`World`] is the storage of record (the active `Scene`), [`Resources`] is
//! the engine singletons. `GameWorld` owns one of each and threads the pair through
//! the schedule. Both halves own PLAIN data — no `Rc`, no `RefCell` (#57): the mlua
//! script bindings no longer capture engine state with a `'static` lifetime; a
//! script run opens a `lua.scope` over the borrowed sim data instead. The borrow
//! checker separates world from resources at the call boundary.

pub mod game;
mod game_play;
mod particles;
mod play;
mod registry;
mod resources;
mod schedule;
mod stage;
mod system;
mod world;

pub use game::{GameWorld, PlayTransition};
pub use registry::{build, App};
pub use resources::Resources;
pub use schedule::Schedule;
pub use stage::Stage;
pub use system::System;
pub use world::World;
