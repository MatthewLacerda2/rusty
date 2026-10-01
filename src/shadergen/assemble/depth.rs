//! src/shadergen/assemble/depth.rs — a cutting surface variant's depth entry points
//! (#648).
//!
//! A block that `discard`s (`dissolve`) cuts the colour pass; the shadow cascades and
//! the SSAO prepass draw the same mesh through their own fragment stages, which would
//! otherwise keep every fragment. So a recipe holding a cutting block gets two more
//! entry points: [`CUT_PREPASS`] and [`CUT_SHADOW`] open like `fs_main` (the UV chain
//! moves `in`'s UVs, `uv`), run each cutting instance's [`Block::cut`] with the same
//! args as its colour call, then the base's own clip for that pass (`prepass_clip`,
//! `shadow_clip` — the cutout test). A recipe with no cutting block gets neither, and
//! its variant keeps the shared depth pipelines.
//!
//! [`Block::cut`]: crate::shadergen::blocks::Block::cut

use std::fmt::Write as _;

use super::emit::{cuts, Instance};
use super::uv::moved_in;
use crate::shadergen::params::ParamLayout;

/// The SSAO prepass entry point of a surface variant that cuts fragments.
pub const CUT_PREPASS: &str = "fs_prepass_cut";
/// The shadow-cascade entry point of a surface variant that cuts fragments.
pub const CUT_SHADOW: &str = "fs_shadow_cut";

/// The depth entry points for `instances`, or nothing when none of them cuts.
pub fn entry_points(instances: &[Instance], layout: &ParamLayout) -> String {
    let cuts = cuts(instances, layout);
    if cuts.is_empty() {
        return String::new();
    }
    let (param, opening) = match moved_in(instances, layout) {
        Some(moved) => ("in_raw", moved),
        None => ("in", String::new()),
    };
    let body: String = cuts.iter().map(|c| format!("    _ = {c};\n")).collect();
    let mut out = String::from("\n// ---- the blocks' cuts in the depth passes (#648) ----\n");
    for (entry, clip) in [(CUT_PREPASS, "prepass_clip"), (CUT_SHADOW, "shadow_clip")] {
        let _ = write!(
            out,
            "@fragment\nfn {entry}({param}: VertexOutput) {{\n{opening}{body}    {clip}(in);\n}}\n"
        );
    }
    out
}
