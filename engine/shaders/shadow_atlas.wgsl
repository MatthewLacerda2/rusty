// engine/shaders/shadow_atlas.wgsl — the point/spot shadow atlas's tile blits (#694).
//
// WebGPU copies depth only as whole subresources and clears only whole attachments,
// so an atlas tile (a sub-rectangle) is copied and cleared by drawing: one
// fullscreen triangle into the tile's viewport, writing `frag_depth`.
//  * `fs_copy`  — the static atlas's texel under the fragment, unchanged: the active
//                 atlas starts each frame from the cached static casters.
//  * `fs_clear` — the far plane (1.0), before a stale tile's statics are re-baked.

#import fullscreen

@group(0) @binding(0)
var t_static: texture_depth_2d;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

// Both atlases are the same size, so a fragment's framebuffer texel is its texel
// in the static atlas too.
@fragment
fn fs_copy(@builtin(position) pos: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(t_static, vec2<i32>(pos.xy), 0);
}

@fragment
fn fs_clear() -> @builtin(frag_depth) f32 {
    return 1.0;
}
