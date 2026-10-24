// engine/shaders/fullscreen.wgsl — the fullscreen triangle every fullscreen pass
// draws (#1322): three vertices with no vertex buffer, covering clip space. A pass
// wraps `triangle` in its own `@vertex` entry point (naga_oil keeps an imported
// module's entry points out of the importer). It is the only place the triangle is
// written: `make rules` (`fullscreen-triangle`, #1370) flags a copy anywhere else.

#define_import_path fullscreen

// Vertex `i` (0, 1, 2) as a texture coordinate: (0,0), (2,0), (0,2). The visible
// part is [0,1]².
fn uv(i: u32) -> vec2<f32> {
    return vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
}

// The clip-space corner of vertex `i` of the triangle, `uv(i)` mapped to [-1,1]².
fn triangle(i: u32) -> vec4<f32> {
    return vec4<f32>(uv(i) * 2.0 - 1.0, 0.0, 1.0);
}

// The same corner with y flipped, so `uv(i)` reads a texture top-down: what a pass
// that samples the previous frame's image through that `uv` wants.
fn triangle_down(i: u32) -> vec4<f32> {
    let p = uv(i);
    return vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
