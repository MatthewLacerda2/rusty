//! src/shadergen/assemble/depth.rs — the depth entry points of a surface variant that
//! cuts fragments or moves its UVs (#648, #1499).
//!
//! A block that `discard`s (`dissolve`) cuts the colour pass; the shadow cascades and
//! the SSAO prepass draw the same mesh through their own fragment stages, which would
//! otherwise keep every fragment. A UV-stage block (`uv_scroll`) moves where the colour
//! pass samples, so its Cutout test reads other texels than the shared depth stages,
//! which sample at the raw UVs. Either way the recipe gets two more entry points:
//! [`CUT_PREPASS`] and [`CUT_SHADOW`] open like `surface_color` (the UV chain moves
//! `in`'s UVs, `uv`), run each cutting instance's [`Block::cut`] with the same args as
//! its colour call, then the base's own clip for that pass (`prepass_clip`,
//! `shadow_clip` — the cutout test). A recipe that neither cuts nor moves its UVs gets
//! neither, and its variant keeps the shared depth pipelines. A primed forward pass
//! (#1499) relies on this: the prepass depth must be the depth `fs_main` keeps.
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

/// The depth entry points for `instances`, or nothing when none of them cuts or
/// moves the UVs.
pub fn entry_points(instances: &[Instance], layout: &ParamLayout) -> String {
    let cuts = cuts(instances, layout);
    let (param, opening) = match moved_in(instances, layout) {
        Some(moved) => ("in_raw", moved),
        None if cuts.is_empty() => return String::new(),
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
