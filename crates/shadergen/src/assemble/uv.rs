use super::emit::{chain, Instance};
use crate::blocks::Stage;
use crate::params::ParamLayout;

const SURFACE_COLOR: &str = "fn surface_color(in: VertexOutput) -> vec4<f32> {";

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

pub fn moved_in(instances: &[Instance], layout: &ParamLayout) -> Option<String> {
    instances
        .iter()
        .any(|(b, _)| b.stage == Stage::Uv)
        .then(|| {
            let uv = chain(instances, "in.tex_coords", layout, Stage::Uv);
            format!("    var in = in_raw;\n    in.tex_coords = {uv};\n")
        })
}
