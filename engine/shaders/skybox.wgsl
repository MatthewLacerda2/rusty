#import common::{CameraUniforms, VertexInput, equirect_uv, sky_fog}
#import sky::motion::{SkyTargets, sky_targets}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.view_dir = model.position;
    
    // We translate the skybox box around the camera position in world space
    let world_pos = model.position + camera.camera_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    
    // Set z to w so it resides exactly at the far plane depth (1.0)
    out.clip_position = out.clip_position.xyww;
    return out;
}

@group(1) @binding(0)
var t_skybox: texture_2d<f32>;
@group(1) @binding(1)
var s_skybox: sampler;

@fragment
fn fs_main(in: VertexOutput) -> SkyTargets {
    let dir = normalize(in.view_dir);
    // Level 0: the panorama shares its cached upload, mips and all, with any
    // material using the file, and `equirect_uv`'s atan2 seam would make the
    // derivatives pick a coarse mip, a flat line down the sky (#1322).
    let sky = textureSampleLevel(t_skybox, s_skybox, equirect_uv(dir), 0.0);
    // Blend toward the scene fog at the horizon (#437) so fogged geometry doesn't
    // silhouette against a clear sky.
    let color = vec4<f32>(sky_fog(camera.fog, sky.rgb, dir, camera.camera_pos), sky.a);
    return sky_targets(camera, color, dir);
}
