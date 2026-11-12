//! src/scene/runtime/ — the scene half of play-mode requests that cannot happen
//! mid-dispatch, so a script only *queues* them and the tick drains them at its tail:
//!
//!   debug_lines   — `Debug.DrawLine`/`DrawRay`'s lines, aged at the tick's head (#1882)
//!   destroy_queue — `Scene.DestroyEntity`'s deferred removal (#323)
//!   despawn       — the one despawn door and the play state guarding it (#1302)
//!   load          — `Scene.Load`'s deferred swap and `DontDestroyOnLoad` (#432)
//!   loads         — the pending load and `Scene.LoadAsync`'s operations (#2123)
//!   prepare       — the file-only half of a load, a value the swap takes (#2123)
//!
//! The script-side teardown (`OnDisable`/`OnDestroy`) lives in `scripting`; the
//! systems that sequence both live in `app::play`.

pub mod debug_lines;
pub mod despawn;
pub mod destroy_queue;
pub mod load;
pub mod loads;
pub mod prepare;

#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod swap_refs_tests;
