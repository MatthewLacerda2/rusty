//! src/api/input.rs — Input API
//!
//! Read (`IsKeyDown`) + WRITABLE (`Press`/`Release`). Writable input is what lets a
//! script play as the user. Unity: `Input`.
//!
//! Allowed deps: input.
//! Status: IMPLEMENTED — `Press`/`Release` in
//! `scripting::bindings::world::register_input_writable` (issue #5);
//! `IsKeyDown` is registered in `scripting::ScriptManager::init_runtime`.
