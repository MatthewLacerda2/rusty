// assets/shaders/ribbons.wgsl — camera-facing ribbons: trails and lines (#441).
//
// The CPU builds each ribbon as a strip of point pairs: both vertices of a pair
// sit on the centre line with the line's tangent, the width and colour at that
// point, and `side` = -1 / +1. The vertex stage pushes each one half the width
// out along `cross(tangent, to_camera)`, so the strip always faces the camera.
// Unlit, fogged like the particles (#437): alpha ribbons fade into the fog
// colour, additive ones fade out.

#import common::{Fog, fog_factor}

struct RibbonGlobals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>, // xyz used
    fog: Fog,
};

@group(0) @binding(0) var<uniform> globals: RibbonGlobals;
@group(1) @binding(0) var ribbon_tex: texture_2d<f32>;
@group(1) @binding(1) var ribbon_sampler: sampler;

struct VertexInput {
    @location(0) center: vec3<f32>,
    @location(1) side: f32,
    @location(2) tangent: vec3<f32>,
    @location(3) width: f32,
    @location(4) color: vec4<f32>,
    @location(5) u: f32,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) fog: f32,
};

// The direction across the ribbon at a point: perpendicular to the line and to
// the view ray. Looking straight down the line, fall back to any perpendicular.
fn across(center: vec3<f32>, tangent: vec3<f32>) -> vec3<f32> {
    let to_camera = globals.cam_pos.xyz - center;
    let c = cross(tangent, to_camera);
    if (dot(c, c) > 1e-12) {
        return normalize(c);
    }
    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(tangent.y) > 0.99);
    return normalize(cross(tangent, up));
}

@vertex
fn vs_main(in: VertexInput) -> VsOut {
    let world = in.center + across(in.center, in.tangent) * (in.width * 0.5 * in.side);
    var out: VsOut;
    out.clip_position = globals.view_proj * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(in.u, in.side * 0.5 + 0.5);
    out.color = in.color;
    out.fog = fog_factor(globals.fog, world, globals.cam_pos.xyz);
    return out;
}

@fragment
fn fs_alpha(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(ribbon_tex, ribbon_sampler, in.uv) * in.color;
    return vec4<f32>(mix(c.rgb, globals.fog.color, in.fog), c.a);
}

@fragment
fn fs_additive(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(ribbon_tex, ribbon_sampler, in.uv) * in.color;
    return vec4<f32>(c.rgb * (1.0 - in.fog), c.a);
}
