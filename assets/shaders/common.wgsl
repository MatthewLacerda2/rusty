// assets/shaders/common.wgsl — shared GPU struct definitions and helpers imported by
// the forward, shadow, skybox, particle and decal passes via `#import "common"`.

// The scene's distance + height fog (#437). Mirrors the Rust `FogUniform`
// byte-for-byte. `mode`: 0 off, 1 linear, 2 exponential, 3 exponential².
struct Fog {
    color: vec3<f32>,
    mode: u32,
    start: f32,
    end: f32,
    density: f32,
    height_falloff: f32,
    base_height: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
    // Rides with the camera so every pass that already binds it fogs for free.
    fog: Fog,
};

// Standard mesh vertex layout — position, normal, UVs, skeletal animation data
// (four joint indices + blend weights; no joint cap, #455), and the tangent basis for normal
// mapping (`xyz` unit tangent, `w` handedness sign).
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
    @location(5) tangent: vec4<f32>,
};

// A vertex's skinning matrix (#599): its four joint matrices blended by `weights`.
// Zero weights (a vertex no joint moves) skin as identity. The one skinning function
// the forward and shadow passes share, so a shadow always follows the drawn pose;
// each pass looks the four matrices up in its own copy of the frame's joint array.
fn blend_joints(
    j0: mat4x4<f32>,
    j1: mat4x4<f32>,
    j2: mat4x4<f32>,
    j3: mat4x4<f32>,
    weights: vec4<f32>,
) -> mat4x4<f32> {
    if (weights.x + weights.y + weights.z + weights.w < 0.01) {
        return mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 1.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        );
    }
    return j0 * weights.x + j1 * weights.y + j2 * weights.z + j3 * weights.w;
}

// How far the sky counts as, for fog: past any level's far wall, so the sky is at
// least as fogged as the farthest geometry in front of it.
const FOG_SKY_DISTANCE: f32 = 1000.0;

// Mean relative fog thickness along a ray whose heights run `a`..`b` above the base
// height: full (1) below it, `e^(-k·h)` above it. Exact integral of that profile,
// so a ray that dips under the base picks up the full layer for that stretch.
fn fog_height_mean(k: f32, a: f32, b: f32) -> f32 {
    let lo = min(a, b);
    let hi = max(a, b);
    if (k <= 0.0) {
        return 1.0;
    }
    if (hi - lo < 1e-3) {
        return exp(-k * max(lo, 0.0));
    }
    let below = min(hi, 0.0) - min(lo, 0.0);
    let above = (exp(-k * max(lo, 0.0)) - exp(-k * max(hi, 0.0))) / k;
    return (below + above) / (hi - lo);
}

// How fogged (0 clear .. 1 fully fog colour) a point is, seen from `eye`. The one
// fog formula every pass uses, so surfaces, particles, decals and the sky agree.
fn fog_factor(fog: Fog, world_pos: vec3<f32>, eye: vec3<f32>) -> f32 {
    if (fog.mode == 0u) {
        return 0.0;
    }
    let d = max(distance(world_pos, eye) - fog.start, 0.0);
    let h = fog_height_mean(fog.height_falloff, eye.y - fog.base_height, world_pos.y - fog.base_height);
    var f: f32;
    if (fog.mode == 1u) {
        f = d * h / max(fog.end - fog.start, 1e-3);
    } else if (fog.mode == 2u) {
        f = 1.0 - exp(-fog.density * d * h);
    } else {
        let x = fog.density * d * h;
        f = 1.0 - exp(-x * x);
    }
    return clamp(f, 0.0, 1.0);
}

// Fade a lit colour into the fog colour — opaque, transparent and alpha-blended
// surfaces (the caller keeps its alpha).
fn apply_fog(fog: Fog, color: vec3<f32>, world_pos: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    return mix(color, fog.color, fog_factor(fog, world_pos, eye));
}

// The sky's fog along view direction `dir`: as fogged as a point at
// FOG_SKY_DISTANCE, faded out above the horizon so the zenith stays sky.
fn sky_fog(fog: Fog, color: vec3<f32>, dir: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    let f = fog_factor(fog, eye + dir * FOG_SKY_DISTANCE, eye);
    let horizon = 1.0 - smoothstep(0.0, 0.4, dir.y);
    return mix(color, fog.color, f * horizon);
}
