// engine/shaders/outline/edge.wgsl — the selection outline's ring (#1175).
//
// One fullscreen triangle over the HDR target: a pixel outside the selection mask
// (`outline/mask.wgsl`) that has a mask pixel within `radius` pixels is the ring.
// It is drawn over everything the camera drew, as Unity's selection outline is.

struct EdgeParams {
    color: vec4<f32>,
    // The ring's width in pixels (the view's pixel scale already applied).
    radius: f32,
};

@group(0) @binding(0)
var<uniform> p: EdgeParams;
@group(0) @binding(1)
var t_mask: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// Whether pixel `px` is in the mask; off the view is not.
fn masked(px: vec2<i32>, dims: vec2<i32>) -> bool {
    if (any(px < vec2<i32>(0)) || any(px >= dims)) {
        return false;
    }
    return textureLoad(t_mask, px, 0).r > 0.5;
}

@fragment
fn fs_edge(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let dims = vec2<i32>(textureDimensions(t_mask));
    var ring = false;
    if (!masked(px, dims)) {
        // Every mask pixel within a disc of `radius` around this one.
        let reach = i32(ceil(p.radius));
        let r2 = p.radius * p.radius;
        for (var y = -reach; y <= reach && !ring; y++) {
            for (var x = -reach; x <= reach && !ring; x++) {
                let d = vec2<f32>(f32(x), f32(y));
                ring = dot(d, d) <= r2 && masked(px + vec2<i32>(x, y), dims);
            }
        }
    }
    if (!ring) {
        discard;
    }
    return p.color;
}
