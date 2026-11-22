use std::fmt::Write as _;

use super::emit::{cuts, Instance};
use super::uv::moved_in;
use super::vertex;
use crate::params::ParamLayout;

pub const CUT_PREPASS: &str = "fs_prepass_cut";
pub const CUT_SHADOW: &str = "fs_shadow_cut";

pub fn entry_points(instances: &[Instance], layout: &ParamLayout) -> String {
    let cuts = cuts(instances, layout);
    let (param, opening) = match moved_in(instances, layout) {
        Some(moved) => ("in_raw", moved),
        None if cuts.is_empty() && !vertex::moves(instances) => return String::new(),
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
