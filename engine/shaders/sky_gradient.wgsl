// engine/shaders/sky_gradient.wgsl — procedural fallback skybox (#256).
//
// Drawn by the skybox pass when a camera would otherwise clear to the flat editor
// backdrop (no panorama texture bound). It paints a vertical sky -> horizon -> ground
// gradient (Unity default-skybox vibe) on the far-plane box, so an empty scene reads
// as a lit environment rather than a flat slab. A cheap static cloud layer (value-noise
// FBM, sky hemisphere only) breaks up the otherwise perfectly clean gradient. Both
// live in `common::procedural_sky`, shared with the forward pass's reflections (#718).
//
// It shares group(0) with the forward pass: camera at binding 0, the SAME lighting
// uniform at binding 1. Its look is the scene's sky settings (`lighting.sky`, #1620);
// a colour they leave unset is derived from `lighting.ambient.color`, the very term
// the surface shader uses for hemisphere ambient, so by default the background and
// the lighting always agree.

#import lighting::ambient::AmbientLight
#import common::{CameraUniforms, ProceduralSky, VertexInput, procedural_sky, sky_fog}
#import sky::motion::{SkyTargets, sky_targets}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// A prefix of the forward pass's `LightingUniforms`: we only need the leading
// `ambient` term and the procedural sky's settings (#1620), and a uniform binding
// may declare a struct smaller than the bound buffer. `AmbientLight` is the shared
// `lighting::ambient` one, so the prefix can't drift from it.
struct SkyLighting {
    ambient: AmbientLight,
    sky: ProceduralSky,
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
fn fs_main(in: VertexOutput) -> SkyTargets {
    let dir = normalize(in.view_dir);
    // The scene's sky settings, tinted from the SAME ambient term the surface shader
    // reads wherever they leave a colour unset; the gradient and clouds live in
    // `common::procedural_sky`, which the forward pass reflects (#718).
    let color = procedural_sky(lighting.sky, lighting.ambient.color, dir);

    // Blend toward the scene fog at the horizon (#437), as the panorama sky does.
    let fogged = vec4<f32>(sky_fog(camera.fog, color, dir, camera.camera_pos), 1.0);
    return sky_targets(camera, fogged, dir);
}
