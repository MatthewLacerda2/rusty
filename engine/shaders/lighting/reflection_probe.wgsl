// engine/shaders/lighting/reflection_probe.wgsl — one resident reflection probe as
// the shader reads it, and its box-projected parallax correction (#244, #1206): the
// WGSL side of `scene::lighting::reflection_probe`. `common::LightingUniforms` holds
// the resident probes; `forward::reflection::environment_reflection` samples them.

#define_import_path lighting::reflection_probe

// One resident reflection probe (#1206): its capture point and its parallax box in
// its volume's frame (#1995: `rotation` turns a world vector into it, identity for a
// world-aligned box, `shape.x` is 1 for a sphere of radius `shape.y`, `shape.z` its
// blend distance and `shape.w` the projection's fade span past it), the
// layer of the probe cube array its cube sits in (probes sharing a cube share it),
// where that cube's prefiltered mip chain (#245) starts, (#1584) its intensity
// (`params.x`) and box-projection switch (`params.y`, 1 on), and (#1697) the
// fallback flag (`params.z`, only ever slot 0), the normalisation switch (`params.w`,
// 1 when `irradiance` is stored) and the luminance of its stored irradiance as 9 L2
// SH coefficients, convolved over pi. Mirrors the Rust `ReflectionProbeUniform`
// byte-for-byte.
struct ReflectionProbe {
    center: vec3<f32>,
    mip_base: f32,
    box_min: vec3<f32>,
    mip_count: f32,
    box_max: vec3<f32>,
    layer: u32,
    params: vec4<f32>,
    irradiance: array<vec4<f32>, 3>,
    rotation: vec4<f32>,
    shape: vec4<f32>,
};

// `v` turned by the quaternion `q` (`xyz`, `w`): an identity `q` adds an exact zero.
// The Rust `reflection_probe::volume::rotate`, step for step.
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

// Box- or sphere-projected parallax correction (#244, #1995), the standard Lagarde
// technique against the probe's oriented volume (HDRP's proxy). Given a fragment
// world position and a raw reflection direction, turn both into the volume's frame,
// find where the ray leaves the volume, and return the world direction from the
// probe centre to that point, so the reflection tracks the room as the camera moves.
// A fragment OUTSIDE the volume (a large object reaching past its probe) projects
// too, each exit distance taken as a magnitude (never negative inside the volume),
// so the result is continuous across every face and every direction instead of
// stepping to infinite projection at a face (the module docs of `projection.rs`
// say why not HDRP's clamp at zero). Past the influence volume (the volume grown by
// `shape.z`) the result fades back to infinite projection over `shape.w`, so a far
// fragment never collapses every R onto one patch of the cube. Runs the same steps as the Rust `ReflectionProbe::parallax_correct`
// (`scene::lighting::reflection_probe::projection`), which is unit-tested without a
// GPU. WGSL may assume no inf or NaN, so an axis the ray runs parallel to is never
// divided by: it gets `PARALLAX_NO_HIT` instead.
const PARALLAX_EPSILON: f32 = 1e-6; // Rust `PARALLEL_EPSILON`
const PARALLAX_NO_HIT: f32 = 1e30; // Rust `NO_HIT`
fn parallax_correct(probe: ReflectionProbe, world_pos: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let p = quat_rotate(probe.rotation, world_pos);
    var outside = 0.0;
    if (probe.shape.x > 0.5) {
        outside = max(length(p - probe.center) - probe.shape.y, 0.0);
    } else {
        outside = distance(clamp(p, probe.box_min, probe.box_max), p);
    }
    // `smoothstep` is undefined for equal edges; the fade span is never below 0.01.
    let fade = smoothstep(probe.shape.z, probe.shape.z + probe.shape.w, outside);
    if (fade >= 1.0) {
        return dir;
    }
    let projected = project_in_volume(probe, p, dir);
    if (fade <= 0.0) {
        return projected;
    }
    let mixed = mix(projected, dir, fade);
    if (dot(mixed, mixed) <= 0.0) {
        return dir;
    }
    return normalize(mixed);
}

// The world direction from the capture point to where the ray from `p` (in the
// volume's frame) along `dir` (world) leaves the volume: the Rust `project_in_volume`.
fn project_in_volume(probe: ReflectionProbe, p: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let d = quat_rotate(probe.rotation, dir);
    var t = PARALLAX_NO_HIT;
    if (probe.shape.x > 0.5) {
        // The far root, or the nearest approach when the ray misses the sphere.
        let oc = p - probe.center;
        let b = dot(oc, d);
        let c = dot(oc, oc) - probe.shape.y * probe.shape.y;
        t = abs(-b + sqrt(max(b * b - c, 0.0)));
    } else {
        let crosses = abs(d) > vec3<f32>(PARALLAX_EPSILON);
        let safe_dir = select(vec3<f32>(1.0), d, crosses);
        let first = (probe.box_max - p) / safe_dir;
        let second = (probe.box_min - p) / safe_dir;
        let exits = select(vec3<f32>(PARALLAX_NO_HIT), abs(max(first, second)), crosses);
        t = min(min(exits.x, exits.y), exits.z);
    }
    if (t >= PARALLAX_NO_HIT) {
        return dir;
    }
    let to_hit = p + d * t - probe.center;
    if (dot(to_hit, to_hit) <= 0.0) {
        return dir;
    }
    return normalize(quat_rotate(vec4<f32>(-probe.rotation.xyz, probe.rotation.w), to_hit));
}
