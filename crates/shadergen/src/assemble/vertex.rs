use super::emit::{chain, Instance};
use crate::blocks::Stage;
use crate::params::ParamLayout;

const SURFACE_VERTEX: &str =
    "fn surface_vertex(v: SurfaceVertex, time: f32) -> SurfaceVertex {\n    return v;\n}";

pub fn moves(instances: &[Instance]) -> bool {
    instances.iter().any(|(b, _)| b.stage == Stage::Vertex)
}

pub fn splice(body: &str, instances: &[Instance], layout: &ParamLayout) -> Result<String, String> {
    if !moves(instances) {
        return Ok(body.to_string());
    }
    if !body.contains(SURFACE_VERTEX) {
        return Err("surface base shader is missing the expected surface_vertex hook".into());
    }
    let moved = chain(instances, "v", layout, Stage::Vertex);
    let hook = SURFACE_VERTEX.replace("return v;", &format!("return {moved};"));
    Ok(body.replacen(SURFACE_VERTEX, &hook, 1))
}
