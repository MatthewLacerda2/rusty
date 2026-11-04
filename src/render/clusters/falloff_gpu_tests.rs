//! The shader's light falloff is the CPU's (#1699): `lighting::falloff`'s
//! `light_falloff` and `spot_cone`, run on the GPU over a table of distances and
//! angles, against `components::falloff` and `components::spot_cone`, which the
//! lightmap and probe bakes light with. A retune of one that left the other behind would bake a
//! lamp's pool of light differently from how it is drawn.

use crate::components::{falloff, falloff_near, spot_cone};

/// The range every falloff row uses, and the spot's cone cosines.
const RANGE: f32 = 5.0;
const COS_INNER: f32 = 0.9;
const COS_OUTER: f32 = 0.6;
/// The near floor the third column runs at.
const NEAR: f32 = 0.5;

/// Per invocation (128): distance `i / 20` (0 to past the range) through the falloff,
/// and cosine `i / 127` through the cone.
const PROBE: &str = "
#import lighting::falloff::{light_falloff, light_falloff_near, spot_cone}
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.y * 64u + id.x;
    let d = f32(i) / 20.0;
    let c = f32(i) / 127.0;
    out[i] = vec4<f32>(light_falloff(d, RANGE), spot_cone(c, COS_INNER, COS_OUTER), light_falloff_near(d, RANGE, NEAR), 0.0);
}
";

fn table(i: u32) -> (f32, f32) {
    (i as f32 / 20.0, i as f32 / 127.0)
}

#[test]
fn gpu_the_shaders_falloff_and_cone_are_the_cpus() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let base = crate::shadergen::engine_shader_dir();
    let mut composer = crate::shadergen::compose::composer_with_common(base).unwrap();
    let source = format!(
        "const RANGE: f32 = {RANGE:?};\nconst COS_INNER: f32 = {COS_INNER:?};\n\
         const COS_OUTER: f32 = {COS_OUTER:?};\nconst NEAR: f32 = {NEAR:?};\n{PROBE}"
    );
    let module =
        crate::shadergen::compose::compose(&mut composer, &source, "falloff_probe").unwrap();
    let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
    let out = crate::render::test_gpu::run_probe(&r.device, &r.queue, source);
    for (i, got) in out.iter().enumerate().take(128) {
        let (d, c) = table(i as u32);
        let want = falloff(d, RANGE);
        assert!(
            (got[0] - want).abs() <= want * 1e-5 + 1e-7,
            "falloff at {d}: GPU {} vs CPU {want}",
            got[0]
        );
        let near = falloff_near(d, RANGE, NEAR);
        assert!(
            (got[2] - near).abs() <= near * 1e-5 + 1e-7,
            "near falloff at {d}: GPU {} vs CPU {near}",
            got[2]
        );
        let cone = spot_cone(c, COS_INNER, COS_OUTER);
        assert!(
            (got[1] - cone).abs() <= 1e-6,
            "cone at {c}: GPU {} vs CPU {cone}",
            got[1]
        );
    }
    // The table spans the edge: lit inside the range, exactly dark past it.
    assert!(out[99][0] > 0.0 && out[100][0] == 0.0 && out[127][0] == 0.0);
}
