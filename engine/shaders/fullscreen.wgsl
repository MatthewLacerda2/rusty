// engine/shaders/fullscreen.wgsl — the fullscreen triangle every fullscreen pass
// draws (#1322): three vertices with no vertex buffer, covering clip space. A pass
// wraps `triangle` in its own `@vertex` entry point (naga_oil keeps an imported
// module's entry points out of the importer). #1370 moves the other passes' copies
// here.

#define_import_path fullscreen

// The clip-space corner of vertex `i` (0, 1, 2) of the triangle.
fn triangle(i: u32) -> vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}
