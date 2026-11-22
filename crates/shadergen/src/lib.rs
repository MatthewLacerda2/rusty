//! stored-format: the baked shaders under `assets/shaders/` and their params files

#![deny(clippy::unwrap_used)]

pub mod assemble;
pub mod bake;
pub mod baked;
pub mod blocks;
pub mod compose;
pub mod groups;
pub mod params;
pub mod post_params;
pub mod rebake;
pub mod recipe;
pub mod textures;
pub mod validate;

pub use bake::{bake_recipe, BakeError};
pub use recipe::{BlockSel, PassKind, ShaderRecipe};

pub fn engine_shader_dir() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| rusty_core::project::engine_path("shaders"))
}

pub const DEFAULT_OUT_DIR: &str = rusty_core::project::SHADERS_DIR;

#[cfg(test)]
mod crt_tests;
#[cfg(test)]
mod gameplay_tests;
#[cfg(test)]
mod instance_tests;
#[cfg(test)]
mod params_tests;
#[cfg(test)]
mod sampling_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texture_tests;
#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod vertex_tests;
