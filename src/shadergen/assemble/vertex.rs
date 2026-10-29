//! src/shadergen/assemble/vertex.rs — the surface vertex stage (#1524).
//!
//! A [`Stage::Vertex`] block moves the vertex itself. The forward base declares
//! `surface_vertex(v, time)` as the identity, and every vertex stage that draws a mesh
//! calls it: `forward_vertex` (so `vs_main` and the SSAO prepass), `vs_velocity` once
//! more at last frame's game time, and `vs_shadow`. A recipe with a vertex block
//! replaces its body with the blocks' chain, so all of them move the vertex alike; one
//! without leaves the base untouched. Such a variant also gets its own depth entry
//! points (`depth`): the standard depth pipelines run the standard, unmoved vertex
//! stage.

use super::emit::{chain, Instance};
use crate::shadergen::blocks::Stage;
use crate::shadergen::params::ParamLayout;

/// The forward base's identity vertex hook, exactly as `shader.wgsl` declares it.
const SURFACE_VERTEX: &str =
    "fn surface_vertex(v: SurfaceVertex, time: f32) -> SurfaceVertex {\n    return v;\n}";

/// Whether any instance moves the vertex.
pub fn moves(instances: &[Instance]) -> bool {
    instances.iter().any(|(b, _)| b.stage == Stage::Vertex)
}

/// Splice the vertex chain into `body`'s `surface_vertex`, or return it as-is when no
/// instance is a vertex block. Errors when the base lacks the expected hook.
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
