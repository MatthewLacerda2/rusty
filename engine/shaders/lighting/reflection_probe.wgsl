// engine/shaders/lighting/reflection_probe.wgsl — one resident reflection probe as
// the shader reads it, and its box-projected parallax correction (#244, #1206): the
// WGSL side of `scene::lighting::reflection_probe`. `common::LightingUniforms` holds
// the resident probes; `forward::lighting::environment_reflection` samples them.

#define_import_path lighting::reflection_probe

// One resident reflection probe (#1206): its capture point, its parallax box, the
// layer of the probe cube array its cube sits in (probes sharing a cube share it),
// and where that cube's prefiltered mip chain (#245) starts. Mirrors the Rust
// `ReflectionProbeUniform` byte-for-byte.
struct ReflectionProbe {
    center: vec3<f32>,
    mip_base: f32,
    box_min: vec3<f32>,
    mip_count: f32,
    box_max: vec3<f32>,
    layer: u32,
};

// Box-projected parallax correction (#244), the standard Lagarde technique. Given a
// fragment world position and a raw reflection direction, intersect the ray against
// `probe`'s box and return the direction from the probe centre to that hit point, so
// the reflection tracks the room as the camera moves. A fragment outside the box (a
// large object reaching past its probe), or a ray with no forward hit, keeps `dir`.
// Runs the same steps as the Rust `ReflectionProbe::parallax_correct`, which is
// unit-tested without a GPU. WGSL may assume no inf or NaN, so an axis the ray runs
// parallel to is never divided by: it gets `PARALLAX_NO_HIT` instead.
const PARALLAX_EPSILON: f32 = 1e-6; // Rust `PARALLEL_EPSILON`
const PARALLAX_NO_HIT: f32 = 1e30; // Rust `NO_HIT`
fn parallax_correct(probe: ReflectionProbe, world_pos: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    if (any(world_pos < probe.box_min) || any(world_pos > probe.box_max)) {
        return dir;
    }
    let crosses = abs(dir) > vec3<f32>(PARALLAX_EPSILON);
    let safe_dir = select(vec3<f32>(1.0), dir, crosses);
    let first = (probe.box_max - world_pos) / safe_dir;
    let second = (probe.box_min - world_pos) / safe_dir;
    let exits = select(vec3<f32>(PARALLAX_NO_HIT), max(first, second), crosses);
    let t = min(min(exits.x, exits.y), exits.z);
    if (t <= 0.0 || t >= PARALLAX_NO_HIT) {
        return dir;
    }
    let to_hit = world_pos + dir * t - probe.center;
    if (dot(to_hit, to_hit) <= 0.0) {
        return dir;
    }
    return normalize(to_hit);
}
