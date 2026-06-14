//! src/api/physics.rs — Physics API
//!
//! velocity/force/kinematic (in `scripting`) + `Physics.Raycast` / `Physics.Shoot`
//! hitscan, lifted out of `app/play.rs` into a reusable cast over the scene.
//!
//! Allowed deps: physics.
//! Status: IMPLEMENTED — Raycast/Shoot in
//! `scripting::bindings::combat::register_physics_hitscan` (issue #5).
