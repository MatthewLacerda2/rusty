//! src/scene/runtime/ — the scene half of play-mode requests that cannot happen
//! mid-dispatch, so a script only *queues* them and the tick drains them at its tail:
//!
//!   destroy_queue — `Scene.DestroyEntity`'s deferred removal (#323)
//!   despawn       — the one despawn door and the play state guarding it (#1302)
//!   load          — `Scene.Load`'s deferred swap and `DontDestroyOnLoad` (#432)
//!
//! The script-side teardown (`OnDisable`/`OnDestroy`) lives in `scripting`; the
//! systems that sequence both live in `app::play`.

pub mod despawn;
pub mod destroy_queue;
pub mod load;

#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod swap_refs_tests;
