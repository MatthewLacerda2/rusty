// One resident reflection probe as the shader reads it, and its box- or sphere-projected
// parallax correction (Lagarde's, against HDRP's oriented proxy volume): the WGSL side of
// `scene::lighting::reflection_probe`, whose `projection.rs` runs the same steps.

#define_import_path lighting::reflection_probe

// Mirrors the Rust `ReflectionProbeUniform` byte for byte. `rotation` turns a world vector into
// the volume's frame (identity for a world-aligned box); `shape.x` is 1 for a sphere of radius
// `shape.y`, `shape.z` the blend distance and `shape.w` the fade span past it. Probes sharing a
// cube share its `layer`; `mip_base` is where its prefiltered chain starts. `params`: x the
// intensity, y box projection (1 on), z the fallback flag (only ever slot 0), w 1 when
// `irradiance` holds the stored irradiance's luminance as 9 L2 SH coefficients, convolved over pi.
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
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

// WGSL may assume no inf or NaN, so an axis the ray runs parallel to is never divided by: it
// exits at `PARALLAX_NO_HIT`. The Rust twins are `PARALLEL_EPSILON` and `NO_HIT`.
const PARALLAX_EPSILON: f32 = 1e-6;
const PARALLAX_NO_HIT: f32 = 1e30;
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

fn project_in_volume(probe: ReflectionProbe, p: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let d = quat_rotate(probe.rotation, dir);
    var t = PARALLAX_NO_HIT;
    if (probe.shape.x > 0.5) {
        let oc = p - probe.center;
        let b = dot(oc, d);
        let c = dot(oc, oc) - probe.shape.y * probe.shape.y;
        t = abs(-b + sqrt(max(b * b - c, 0.0)));
    } else {
        let crosses = abs(d) > vec3<f32>(PARALLAX_EPSILON);
        let safe_dir = select(vec3<f32>(1.0), d, crosses);
        let first = (probe.box_max - p) / safe_dir;
        let second = (probe.box_min - p) / safe_dir;
        // Each exit is a magnitude, so an outside fragment projects continuously across every
        // face, never stepping to infinite projection.
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
