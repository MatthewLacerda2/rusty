
#import common::{AmbientLight, CameraUniforms, ProceduralSky, VertexInput, procedural_sky, sky_fog}
#import sky::air::through_air
#import sky::motion::{SkyTargets, sky_targets}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// A prefix of the forward pass's `LightingUniforms`: we only need the leading `ambient` term
// and the procedural sky's settings, and a uniform binding may declare a struct smaller than
// the bound buffer.
struct SkyLighting {
    ambient: AmbientLight,
    sky: ProceduralSky,
};
@group(0) @binding(1)
var<uniform> lighting: SkyLighting;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
    @location(1) clip: vec4<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.view_dir = model.position;

    // Translate the box around the camera, then force z = w so it sits exactly on the far plane
    // (depth 1.0) and only fills pixels no geometry has drawn over.
    let world_pos = model.position + camera.camera_pos;
    out.clip_position = (camera.view_proj * vec4<f32>(world_pos, 1.0)).xyww;
    out.clip = out.clip_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> SkyTargets {
    let dir = normalize(in.view_dir);
    let color = procedural_sky(lighting.sky, lighting.ambient.color, dir);

    let hazed = sky_fog(camera.fog, color, dir, camera.camera_pos);
    let fogged = vec4<f32>(through_air(hazed, camera.volumetric, in.clip), 1.0);
    return sky_targets(camera, fogged, dir);
}
