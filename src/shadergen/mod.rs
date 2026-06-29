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
//! in **text**, not pixels: a shader recipe selects a base pass kind (surface or
//! postfx) and a list of blocks/params, and the assembler concatenates their WGSL
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
//! - [`validate`] — module string → composed naga module via `naga_oil`, reusing
//!   the engine's `ShaderRegistry` compose path; the gate that stops bad shaders.
//! - [`bake`] — assemble + validate, then write `<dir>/<name>.wgsl`.

pub mod assemble;
pub mod bake;
pub mod blocks;
pub mod recipe;
pub mod validate;

pub use bake::{bake_recipe, BakeError};
pub use recipe::{BlockSel, PassKind, ShaderRecipe};

/// The engine's committed shader set — read-only at bake time: it supplies the
/// surface base (`shader.wgsl`) and the `common` module a bake validates against.
pub const ENGINE_SHADER_DIR: &str = "assets/shaders";

/// The default output dir for authored shaders: the gitignored project workspace,
/// the same pattern as authored textures/scripts. A `ShaderRegistry` pointed here
/// loads a baked variant by name; bakes never touch the committed engine set.
pub const DEFAULT_OUT_DIR: &str = "project/assets/shaders";

#[cfg(test)]
mod tests;
