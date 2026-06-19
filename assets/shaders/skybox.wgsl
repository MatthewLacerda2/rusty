#import common::{CameraUniforms, VertexInput}

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

const PI: f32 = 3.14159265359;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.view_dir);
    // Convert 3D direction to spherical texture mapping (panoramic projection)
    let phi = atan2(dir.z, dir.x);
    let theta = acos(dir.y);
    let u = (phi + PI) / (2.0 * PI);
    let v = theta / PI;
    return textureSample(t_skybox, s_skybox, vec2<f32>(u, v));
}
