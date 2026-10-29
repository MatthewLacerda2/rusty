//! The headless bake entry points: Generate Lighting (every bake a scene has, in
//! order), the one-button lighting bake, the lightmap bake, and the probe and
//! reflection-probe bakes.

pub mod generate_lighting;
pub mod lighting;
pub mod lightmap;
pub mod probe;
pub mod reflection;

#[cfg(test)]
mod open_world_tests;
