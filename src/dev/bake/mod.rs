//! The headless bake entry points: Generate Lighting (every bake a scene has, in
//! order), the one-button lighting bake, the lightmap bake, and the probe and
//! reflection-probe bakes, plus the log a script's bake reports its progress to
//! (`progress`, #1667).

pub mod generate_lighting;
pub mod lighting;
pub mod lightmap;
pub mod probe;
pub mod progress;
pub mod reflection;
pub mod steps;

#[cfg(test)]
mod open_world_tests;
