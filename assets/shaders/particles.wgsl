// assets/shaders/particles.wgsl — camera-facing 2D billboard particles (issue #13).
//
// Each particle is one instance; the vertex shader expands it into a quad that
// always faces the camera by offsetting the world-space center along the camera's
// right/up axes (passed in the uniform). Per-particle size and RGBA tint come from
// the instance buffer. The fragment shader samples the sprite texture and
// multiplies by the tint; alpha/additive blending is selected by the pipeline.

struct ParticleGlobals {
    view_proj: mat4x4<f32>,
    cam_right: vec4<f32>, // xyz used
    cam_up: vec4<f32>,    // xyz used
};

@group(0) @binding(0) var<uniform> globals: ParticleGlobals;
@group(1) @binding(0) var sprite_tex: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

struct InstanceInput {
    @location(0) center: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

// Corner of a unit quad (two triangles via the index buffer 0,1,2,0,2,3) derived
// arithmetically from the vertex index so no dynamically-indexed array is needed
// (naga forbids indexing a const array by a non-constant). Corners:
//   0:(-0.5,-0.5) 1:(0.5,-0.5) 2:(0.5,0.5) 3:(-0.5,0.5)
// Winding is irrelevant — the pipeline disables culling (cull_mode: None).
fn corner_of(vid: u32) -> vec2<f32> {
    let x = select(-0.5, 0.5, vid == 1u || vid == 2u);
    let y = select(-0.5, 0.5, vid == 2u || vid == 3u);
    return vec2<f32>(x, y);
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceInput) -> VsOut {
    let corner = corner_of(vid);
    // UV: x follows the +0.5 side, y is flipped so the sprite is upright.
    let uv = vec2<f32>(corner.x + 0.5, 0.5 - corner.y);
    let right = globals.cam_right.xyz;
    let up = globals.cam_up.xyz;
    let world = inst.center
        + right * (corner.x * inst.size)
        + up * (corner.y * inst.size);

    var out: VsOut;
    out.clip_position = globals.view_proj * vec4<f32>(world, 1.0);
    out.uv = uv;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex = textureSample(sprite_tex, sprite_sampler, in.uv);
    return tex * in.color;
}
