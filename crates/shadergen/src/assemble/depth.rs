use std::fmt::Write as _;

use super::emit::{cuts, Instance};
use super::uv::moved_in;
use super::vertex;
use crate::params::ParamLayout;

/// `shader.wgsl`'s SSR translucent prepass, character for character.
const SSR_SURFACE: &str = "fn fs_ssr_transparent(in: VertexOutput) -> @location(0) vec4<f32> {";

pub const CUT_PREPASS: &str = "fs_prepass_cut";
pub const CUT_SHADOW: &str = "fs_shadow_cut";

/// The shared depth pipelines never cut, clip at the raw UVs and draw the unmoved vertex,
/// so a variant that cuts or moves its UVs or vertices gets its own: the prepass must keep
/// `fs_main`'s depth.
pub fn entry_points(instances: &[Instance], layout: &ParamLayout) -> String {
    let cuts = cuts(instances, layout);
    let (param, opening) = match moved_in(instances, layout) {
        Some(moved) => ("in_raw", moved),
        None if cuts.is_empty() && !vertex::moves(instances) => return String::new(),
        None => ("in", String::new()),
    };
    let body: String = cuts.iter().map(|c| format!("    _ = {c};\n")).collect();
    let mut out = String::from("\n// ---- the blocks' cuts in the depth passes ----\n");
    for (entry, clip) in [(CUT_PREPASS, "prepass_clip"), (CUT_SHADOW, "shadow_clip")] {
        let _ = write!(
            out,
            "@fragment\nfn {entry}({param}: VertexOutput) {{\n{opening}{body}    {clip}(in);\n}}\n"
        );
    }
    out
}

/// SSR's translucent prepass draws the surface the forward pass shades: the same moved UVs
/// and the same cuts.
pub fn ssr_surface(
    body: &str,
    instances: &[Instance],
    layout: &ParamLayout,
) -> Result<String, String> {
    let cuts = cuts(instances, layout);
    let moved = moved_in(instances, layout);
    if cuts.is_empty() && moved.is_none() {
        return Ok(body.to_string());
    }
    if !body.contains(SSR_SURFACE) {
        return Err(
            "surface base shader is missing the expected fs_ssr_transparent signature".into(),
        );
    }
    let param = if moved.is_some() { "in_raw" } else { "in" };
    let cut: String = cuts.iter().map(|c| format!("    _ = {c};\n")).collect();
    let opening = moved.unwrap_or_default();
    let opened = format!(
        "fn fs_ssr_transparent({param}: VertexOutput) -> @location(0) vec4<f32> {{\n{opening}{cut}"
    );
    Ok(body.replacen(SSR_SURFACE, &opened, 1))
}
