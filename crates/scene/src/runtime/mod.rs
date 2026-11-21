//! crates/scene/src/runtime/ — the scene half of play-mode requests that cannot happen
//! mid-dispatch, so a script only *queues* them and the tick drains them at its tail:
//!
//!   debug_lines   — `Debug.DrawLine`/`DrawRay`'s lines, aged at the tick's head (#1882)
//!   destroy_queue — `Scene.DestroyEntity`'s deferred removal (#323)
//!   despawn       — the one despawn door and the play state guarding it (#1302)
//!   load          — `Scene.Load`'s request and `DontDestroyOnLoad` (#432)
//!   loads         — the pending load and `Scene.LoadAsync`'s operations (#2123)
//!
//! The swap itself and the file-only half of a load it takes are the edit side's
//! (`rusty_edit::load`): the swap applies the document through the authoring holds.
//!
//! The script-side teardown (`OnDisable`/`OnDestroy`) lives in `scripting`; the
//! systems that sequence both live in `app::play`.

pub mod debug_lines;
pub mod despawn;
pub mod destroy_queue;
pub mod load;
pub mod loads;
pub mod renumber;
