//! src/shadergen — agent-composed WGSL shader authoring (#272).
//!
//! The agent composes a curated set of **WGSL building blocks** into a serde
//! **recipe**, an **assembler** templates the chosen blocks into a complete
//! `.wgsl` module that conforms to the engine's existing pass + bind-group
//! contract, a **validate** step composes that module through `naga_oil`
//! (GPU-free, the same path the engine loads it by) so a bad shader is caught at
//! authoring time and never ships, and a **bake** step writes the validated
//! module to the shaders dir, registered by name.
//!
//! This is the third and heaviest of the #174 authoring legs. It reuses the
//! shared-spine *spirit* of #270 (recipe → assemble → validate → bake) but works
//! in **text**, not pixels: a shader recipe selects a base pass kind (surface,
//! postfx or ui) and a list of blocks/params, and the assembler concatenates their WGSL
//! snippets into a module. It is distinct from [`crate::procgen`] (textures) and
//! [`crate::scene::authoring`] (entities/components); this authors *shaders*.
//!
//! **Bounded by design.** This is authoring shaders that *fit the engine*, not a
//! general-purpose shader compiler: every block already speaks the engine's
//! structs/bindings (`#import "common"`, the right bind groups, the right entry
//! points), and the agent composes only from the curated library — there is no
//! free-form WGSL synthesis. New render features/passes/bindings are out of scope.
//!
//! Module layout:
//! - [`recipe`] — the serde document (`ShaderRecipe`, `PassKind`, `BlockSel`):
//!   the source of truth, round-trips losslessly.
//! - [`blocks`] — the curated WGSL building-block library (snippet data) plus the
//!   per-pass block catalog.
//! - [`assemble`] — recipe → a complete, contract-conformant `.wgsl` module
//!   string (deterministic; same recipe → byte-identical WGSL).
//! - [`compose`] — the one GPU-free `naga_oil` composition path (#722): the
//!   engine's `ShaderRegistry` and every authored-shader loader compile through it.
//! - [`validate`] — module string → composed naga module through [`compose`], the
//!   same path the engine loads it by; the gate that stops bad shaders.
//! - [`bake`] — assemble + validate, then write `<dir>/<name>.wgsl`.
//! - [`params`] — a surface shader's runtime params (#399): which block params a
//!   material sets from scripts, their uniform slots, and the `<name>.params.json`
//!   sidecar that records them.
//! - [`post_params`] — the runtime params of a volume's postfx chain (#671): one
//!   name resolved across every effect the volume runs.
//! - [`textures`] — the extra texture slots a surface block may sample (#400): a
//!   material-named pattern such as a baked noise map.

pub mod assemble;
pub mod bake;
pub mod blocks;
pub mod compose;
pub mod params;
pub mod post_params;
pub mod recipe;
pub mod textures;
pub mod validate;

pub use bake::{bake_generation, bake_recipe, BakeError};
pub use recipe::{BlockSel, PassKind, ShaderRecipe};

/// The engine's committed shader set — read-only at bake time: it supplies the
/// surface base (`shader.wgsl`) and the `common` module a bake validates against.
/// Absolute, so it resolves from any project (#829): see `core::project::engine_dir`.
pub fn engine_shader_dir() -> &'static str {
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| crate::core::project::engine_path("shaders"))
}

/// The default output dir for authored shaders, relative to the project root: the
/// same pattern as authored textures/scripts. A material naming a baked surface
/// variant renders with it (#396); bakes never touch the engine's set.
pub const DEFAULT_OUT_DIR: &str = "assets/shaders";

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
