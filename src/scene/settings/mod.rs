//! src/scene/settings/ — the project- and scene-wide settings a scene carries.
//!
//! `fog` (the scene's distance + height fog), `layers` and `collision_matrix` (the
//! project-wide Layers registry and its collision table), and `shader_overrides`
//! (per-entity runtime shader params) and `sky` (the procedural sky's look). `scene` re-exports each module.

pub mod collision_matrix;
pub mod fog;
pub mod layers;
pub mod shader_overrides;
pub mod sky;
