// engine/shaders/outline/edge.wgsl — the selection outline's ring (#1175).
//
// A pixel outside the selection mask (`outline/mask.wgsl`) with a mask pixel within
// `radius` pixels is the ring. That disc test is split in two fullscreen passes,
// each scissored to the selection's bounds on screen, so a pixel loads about
// 2·(2r+1) texels instead of (2r+1)²:
//
// 1. `fs_dilate_x` writes, per pixel, the distance to the nearest mask pixel on its
//    row within `radius` (in whole pixels, stored as k/255; 255 for none) into a
//    one-channel target cleared to "none".
// 2. `fs_edge` walks its column: a row `dy` away whose row distance `dx` has
//    dx² + dy² <= radius² is a mask pixel inside the disc. It draws over the
//    post-FX output (#1224), as Unity's selection outline draws over the finished
//    scene: no tonemap, grade, bloom, SSR, motion blur or FXAA touches the ring.

#import fullscreen

struct EdgeParams {
    color: vec4<f32>,
    // The ring's width in pixels (the view's pixel scale already applied).
    radius: f32,
};

// No mask pixel within reach on the row.
const NONE: f32 = 255.0;

@group(0) @binding(0)
var<uniform> p: EdgeParams;
// What the pass reads along its axis: the mask (`fs_dilate_x`) or the row
// distances (`fs_edge`).
@group(0) @binding(1)
var t_source: texture_2d<f32>;
@group(0) @binding(2)
var t_mask: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

fn inside(px: vec2<i32>, dims: vec2<i32>) -> bool {
    return all(px >= vec2<i32>(0)) && all(px < dims);
}

// Whether pixel `px` is in the mask; off the view is not.
fn masked(px: vec2<i32>) -> bool {
    let dims = vec2<i32>(textureDimensions(t_mask));
    return inside(px, dims) && textureLoad(t_mask, px, 0).r > 0.5;
}

@fragment
fn fs_dilate_x(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let reach = i32(ceil(p.radius));
    var nearest = NONE;
    for (var dx = -reach; dx <= reach; dx++) {
        if (masked(px + vec2<i32>(dx, 0))) {
            nearest = min(nearest, f32(abs(dx)));
        }
    }
    return vec4<f32>(nearest / 255.0, 0.0, 0.0, 1.0);
}

@fragment
fn fs_edge(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let dims = vec2<i32>(textureDimensions(t_source));
    let reach = i32(ceil(p.radius));
    let r2 = p.radius * p.radius;
    var ring = false;
    if (!masked(px)) {
        for (var dy = -reach; dy <= reach && !ring; dy++) {
            let row = px + vec2<i32>(0, dy);
            if (inside(row, dims)) {
                let dx = round(textureLoad(t_source, row, 0).r * 255.0);
                let d = f32(dy);
                ring = dx < NONE && dx * dx + d * d <= r2;
            }
        }
    }
    if (!ring) {
        discard;
    }
    return p.color;
}
