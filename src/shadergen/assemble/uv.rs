//! src/shadergen/assemble/uv.rs — the surface UV stage (#401).
//!
//! A [`Stage::Uv`] block moves the surface's UVs before anything samples them. WGSL
//! function parameters are immutable, so when a recipe holds one the forward base's
//! `surface_color(in: …)` is renamed to take `in_raw` and opens with a mutable copy whose
//! `tex_coords` the UV chain rewrites. The rest of the body reads `in` unchanged, so
//! every map lookup and every color block sees the moved UVs. A recipe with no UV
//! block leaves the base untouched. A variant's depth entry points (#648, #1499)
//! open the same way, so their cut samples the mask where the colour pass does.

use super::emit::{chain, Instance};
use crate::shadergen::blocks::Stage;
use crate::shadergen::params::ParamLayout;

/// The forward base's surface colour function, exactly as `shader.wgsl` declares it:
/// both colour entry points (`fs_main`, the motion-vector `fs_velocity`, #1435) call it.
const SURFACE_COLOR: &str = "fn surface_color(in: VertexOutput) -> vec4<f32> {";

/// Splice the UV chain into `body`'s `surface_color`, or return it as-is when no
/// instance is a UV block. Errors when the base lacks the expected signature.
pub fn splice(body: &str, instances: &[Instance], layout: &ParamLayout) -> Result<String, String> {
    let Some(moved) = moved_in(instances, layout) else {
        return Ok(body.to_string());
    };
    if !body.contains(SURFACE_COLOR) {
        return Err("surface base shader is missing the expected surface_color signature".into());
    }
    let opened = format!("fn surface_color(in_raw: VertexOutput) -> vec4<f32> {{\n{moved}");
    Ok(body.replacen(SURFACE_COLOR, &opened, 1))
}

/// The statements that open an entry point taking `in_raw`: `in`, a mutable copy
/// whose UVs the chain has moved. `None` when no instance is a UV block.
pub fn moved_in(instances: &[Instance], layout: &ParamLayout) -> Option<String> {
    instances
        .iter()
        .any(|(b, _)| b.stage == Stage::Uv)
        .then(|| {
            let uv = chain(instances, "in.tex_coords", layout, Stage::Uv);
            format!("    var in = in_raw;\n    in.tex_coords = {uv};\n")
        })
}
