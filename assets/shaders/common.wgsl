// assets/shaders/common.wgsl — shared GPU struct definitions and helpers imported by
// the forward, shadow, skybox, particle, decal and post-FX passes via `#import "common"`.

// The scene's distance + height fog (#437). Mirrors the Rust `FogUniform`
// byte-for-byte (naga_oil rejects imported names ending `_<digit>`, hence `_pad_a`).
// `mode`: 0 off, 1 linear, 2 exponential, 3 exponential².
struct Fog {
    color: vec3<f32>,
    mode: u32,
    start: f32,
    end: f32,
    density: f32,
    height_falloff: f32,
    base_height: f32,
    _pad_a: f32,
    _pad_b: f32,
    _pad_c: f32,
};

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    // The sim's game time in seconds (#398): scaled by the time scale, frozen while
    // paused, 0 in edit mode and previews. Fills vec3 padding, so no layout change.
    time: f32,
    // Rides with the camera so every pass that already binds it fogs for free.
    fog: Fog,
};

// The post-process chain's uniform (group 0, binding 0 of every post pass, built-in
// and authored). Mirrors the Rust `PostParams`.
struct PostParams {
    // x: exposure (EV), y: contrast, z: saturation, w: gamma
    color: vec4<f32>,
    // x: bloom_intensity, y: bloom_threshold, z: tonemap index, w: bloom_enabled
    bloom: vec4<f32>,
    // x: blur direction (0=horizontal,1=vertical), y: texel_x, z: texel_y, w: motion_blur_samples
    misc: vec4<f32>,
    // x: ssr_mode (0 off, 1 cubemap, 2 screen-space), y: motion_blur_active,
    // z: motion_blur_scale, w: unused
    flags: vec4<f32>,
    // current inverse view-projection (reconstruct world pos from depth)
    inv_view_proj: mat4x4<f32>,
    // previous view-projection (camera motion blur velocity)
    prev_view_proj: mat4x4<f32>,
    // current view-projection (SSR ray marching to clip space)
    view_proj: mat4x4<f32>,
    // xyz: camera world position, w: the sim's game time in seconds (#398) — the
    // same clock as `CameraUniforms.time`, 0 in edit mode and previews
    camera_pos: vec4<f32>,
};

// The forward lighting uniform (group 0, binding 1) and its light records. Shared
// so every pass that shades against the scene lights — the forward pass and lit
// particles (#440) — reads the one layout. Mirrors the Rust `LightingUniform`.
struct AmbientLight {
    color: vec3<f32>,
    intensity: f32,
};

struct DirectionalLight {
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    _pad: f32,
};

struct PointLight {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
};

struct Spotlight {
    position: vec3<f32>,
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    inner_cone: f32, // Cosine of inner angle
    outer_cone: f32, // Cosine of outer angle
};

struct LightingUniforms {
    ambient: AmbientLight,
    dir_light: DirectionalLight,
    point_lights: array<PointLight, 4>,
    spot_light: Spotlight,
    num_point_lights: u32,
    ssr_active: f32,
    ssr_quality: f32,
    ssr_temporal_upsampling: f32,
    // Active reflection probe (#244): when `refl_active > 0.5` the env reflection is
    // box-projected (Lagarde parallax) against this probe's box around `refl_center`
    // instead of sampled as an infinitely-distant skybox. Mirrors the Rust
    // `LightingUniform` byte-for-byte; the `.w` lanes are padding.
    refl_active: f32,
    // 1.0 when the active probe has a baked, prefiltered cubemap bound at binding 4 (#245):
    // the env reflection samples THAT cube (roughness -> mip) with parallax correction
    // instead of the 2D skybox. 0.0 falls back to the skybox. Mirrors the Rust uniform.
    refl_has_cubemap: f32,
    _refl_pad_a: f32,
    _refl_pad_b: f32,
    refl_center: vec4<f32>,
    refl_box_min: vec4<f32>,
    refl_box_max: vec4<f32>,
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
