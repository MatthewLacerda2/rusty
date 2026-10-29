// engine/shaders/opaque_copy.wgsl — the opaque copy (#1523), Unity's *Opaque
// Texture* and *Depth Texture* in one texel: the camera's opaque colour in rgb, and
// in alpha the distance from its eye to the opaque surface there (`SKY_DISTANCE`
// where nothing was drawn). One texture, so the forward pass stays inside its 16
// sampled textures; a refracting or absorbing surface reads it
// (`forward/transparent.wgsl`).

#import fullscreen
#import common::CameraUniforms

// Where nothing was drawn: the largest finite `f16`, the copy's format.
const SKY_DISTANCE: f32 = 65504.0;

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var t_scene: texture_2d<f32>;
@group(0) @binding(2) var t_depth: texture_depth_2d;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

@fragment
fn fs_copy(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(pos.xy);
    let color = textureLoad(t_scene, texel, 0).rgb;
    let depth = textureLoad(t_depth, texel, 0);
    let uv = pos.xy / vec2<f32>(textureDimensions(t_depth));
    let world = camera.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let distance = distance(world.xyz / world.w, camera.camera_pos);
    return vec4<f32>(color, select(min(distance, SKY_DISTANCE), SKY_DISTANCE, depth >= 1.0));
}
