//! src/api/time.rs — Time API
//!
//! `Time.deltaTime/fixedDeltaTime/frameCount`, backed by the `time::Time`
//! resource the `GameWorld` advances once per tick.
//!
//! Allowed deps: time.
//! Status: IMPLEMENTED — Lua bindings in
//! `scripting::bindings::world::register_time` (issue #5).
