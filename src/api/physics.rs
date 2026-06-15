//! src/api/physics.rs — Physics API
//!
//! velocity/force/kinematic (in `scripting`) + `Physics.Raycast` / `Physics.Shoot`
//! hitscan, cast against the live rapier/parry `PhysicsWorld` — the same query
//! pipeline the engine hitscan uses, so script and engine casts agree (#31).
//!
//! Allowed deps: physics.
//! Status: IMPLEMENTED — Raycast/Shoot in
//! `scripting::bindings::combat::register_physics_hitscan` (issues #5, #31).
