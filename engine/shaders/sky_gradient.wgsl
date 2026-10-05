// assets/shaders/sky_gradient.wgsl — procedural fallback skybox (#256).
//
// Drawn by the skybox pass when a camera would otherwise clear to the flat editor
// backdrop (no panorama texture bound). It paints a vertical sky -> horizon -> ground
// gradient (Unity default-skybox vibe) on the far-plane box, so an empty scene reads
// as a lit environment rather than a flat slab. A cheap static cloud layer (value-noise
// FBM, sky hemisphere only) breaks up the otherwise perfectly clean gradient. Both
// live in `common::procedural_sky`, shared with the forward pass's reflections (#718).
//
// It shares group(0) with the forward pass: camera at binding 0, the SAME lighting
// uniform at binding 1. The sky/ground tints are derived from `lighting.ambient.color`
// — the very term the surface shader uses for hemisphere ambient — so the background
// and the lighting always agree (no second source of colour to drift out of sync).

#import common::{CameraUniforms, VertexInput, procedural_sky, sky_fog}

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
    // Sky tint from the SAME ambient term the surface shader reads; the gradient and
    // clouds live in `common::procedural_sky`, which the forward pass reflects (#718).
    let color = procedural_sky(lighting.ambient.color, dir);

    // Blend toward the scene fog at the horizon (#437), as the panorama sky does.
    return vec4<f32>(sky_fog(camera.fog, color, dir, camera.camera_pos), 1.0);
}
