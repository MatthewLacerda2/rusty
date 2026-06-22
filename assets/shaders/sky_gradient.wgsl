// assets/shaders/sky_gradient.wgsl — procedural fallback skybox (#256).
//
// Drawn by the skybox pass when a camera would otherwise clear to the flat editor
// backdrop (no panorama texture bound). It paints a vertical sky -> horizon -> ground
// gradient (Unity default-skybox vibe) on the far-plane box, so an empty scene reads
// as a lit environment rather than a flat slab.
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

    return vec4<f32>(color, 1.0);
}
