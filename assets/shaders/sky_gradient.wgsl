// assets/shaders/sky_gradient.wgsl — procedural fallback skybox (#256).
//
// Drawn by the skybox pass when a camera would otherwise clear to the flat editor
// backdrop (no panorama texture bound). It paints a vertical sky -> horizon -> ground
// gradient (Unity default-skybox vibe) on the far-plane box, so an empty scene reads
// as a lit environment rather than a flat slab. A cheap static cloud layer (value-noise
// FBM, sky hemisphere only) breaks up the otherwise perfectly clean gradient.
//
// It shares group(0) with the forward pass: camera at binding 0, the SAME lighting
// uniform at binding 1. The sky/ground tints are derived from `lighting.ambient.color`
// — the very term the surface shader uses for hemisphere ambient — so the background
// and the lighting always agree (no second source of colour to drift out of sync).

#import common::{CameraUniforms, VertexInput}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// A prefix of the forward pass's `LightingUniforms`: we only need the leading
// `ambient` term, and a uniform binding may declare a struct smaller than the bound
// buffer. Mirrors `AmbientLight` in shader.wgsl byte-for-byte.
struct AmbientLight {
    color: vec3<f32>,
    intensity: f32,
};
struct SkyLighting {
    ambient: AmbientLight,
};
@group(0) @binding(1)
var<uniform> lighting: SkyLighting;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.view_dir = model.position;

    // Translate the box around the camera, then force z = w so it sits exactly on the
    // far plane (depth 1.0) and only fills pixels no geometry has drawn over.
    let world_pos = model.position + camera.camera_pos;
    out.clip_position = (camera.view_proj * vec4<f32>(world_pos, 1.0)).xyww;
    return out;
}

// --- Procedural clouds -------------------------------------------------------
// A cheap static cloud layer so an empty scene doesn't read as a perfectly clean
// gradient. Value-noise FBM sampled on a "sky dome" projection (dir.xz / dir.y),
// which naturally compresses the cloud cells toward the horizon. No time uniform
// is bound here, so the layer is static — just texture, not weather.

fn hash2(p: vec2<f32>) -> f32 {
    // Deterministic 2D -> 1D hash in [0, 1). No engine RNG: this is the platform
    // (render) layer, and the result is a pure function of direction.
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Smoothstep interpolation weights for C1-continuous cells.
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i + vec2<f32>(0.0, 0.0));
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = p;
    for (var i = 0; i < 5; i = i + 1) {
        sum = sum + amp * value_noise(freq);
        freq = freq * 2.0;
        amp = amp * 0.5;
    }
    return sum;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.view_dir);

    // Sky (up) and ground (down) tints, from the SAME ambient term the surface shader
    // reads; ground is the darker quarter exactly as in shader.wgsl's hemisphere mix.
    let sky_color = lighting.ambient.color;
    let ground_color = sky_color * 0.25;
    // A bright horizon band (the classic default-skybox glow), a desaturated lift of
    // the sky tint toward white where it meets the ground line.
    let horizon_color = mix(sky_color, vec3<f32>(1.0), 0.6);

    // Blend ground -> horizon below the horizon line, horizon -> sky above it. The
    // `pow` tightens each band so the horizon glow stays near y = 0.
    let up = clamp(dir.y, 0.0, 1.0);
    let down = clamp(-dir.y, 0.0, 1.0);
    var color = mix(horizon_color, sky_color, pow(up, 0.5));
    color = mix(color, ground_color, pow(down, 0.35));

    // Cloud layer, sky hemisphere only. Project the view ray onto a flat sky dome so
    // cells bunch up toward the horizon, then carve soft cloud shapes out of the FBM
    // and tint them a hair brighter than the sky. Coverage fades out near the horizon
    // (so clouds don't bleed into the glow) and is absent below it.
    if (dir.y > 0.0) {
        let dome = dir.xz / max(dir.y, 0.15);
        let density = fbm(dome * 1.6);
        // smoothstep keeps blue gaps between puffs; the upper bound softens the edges.
        let coverage = smoothstep(0.45, 0.80, density);
        // Fade in from the horizon and back out near the zenith for a believable band.
        let band = smoothstep(0.05, 0.35, up);
        let cloud = coverage * band * 0.6;
        let cloud_color = mix(sky_color, vec3<f32>(1.0), 0.85);
        color = mix(color, cloud_color, cloud);
    }

    return vec4<f32>(color, 1.0);
}
