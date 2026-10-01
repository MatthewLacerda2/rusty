//! src/shadergen/assemble/uv.rs — the surface UV stage (#401).
//!
//! A [`Stage::Uv`] block moves the surface's UVs before anything samples them. WGSL
//! function parameters are immutable, so when a recipe holds one the forward base's
//! `fs_main(in: …)` is renamed to take `in_raw` and opens with a mutable copy whose
//! `tex_coords` the UV chain rewrites. The rest of the body reads `in` unchanged, so
//! every map lookup and every color block sees the moved UVs. A recipe with no UV
//! block leaves the base untouched. A cutting variant's depth entry points (#648)
//! open the same way, so their cut samples the mask where the colour pass does.

use super::emit::{chain, Instance};
use crate::shadergen::blocks::Stage;
use crate::shadergen::params::ParamLayout;

/// The forward base's fragment entry point, exactly as `shader.wgsl` declares it.
const FS_MAIN: &str = "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {";

/// Splice the UV chain into `body`'s `fs_main`, or return it as-is when no
/// instance is a UV block. Errors when the base lacks the expected signature.
pub fn splice(body: &str, instances: &[Instance], layout: &ParamLayout) -> Result<String, String> {
    let Some(moved) = moved_in(instances, layout) else {
        return Ok(body.to_string());
    };
    if !body.contains(FS_MAIN) {
        return Err("surface base shader is missing the expected fs_main signature".into());
    }
    let opened = format!("fn fs_main(in_raw: VertexOutput) -> @location(0) vec4<f32> {{\n{moved}");
    Ok(body.replacen(FS_MAIN, &opened, 1))
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
