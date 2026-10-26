// engine/shaders/postfx/params.wgsl — the post-process chain's uniform, split out of
// `common.wgsl` (#1439) as its own module: every post pass binds it, built-in
// (`postfx.wgsl`) and authored (the `Shader.Bake` postfx scaffold, which reads the
// game time from it, #398).

#define_import_path postfx::params

// The post-process chain's uniform (group 0, binding 0 of every post pass, built-in
// and authored). Mirrors the Rust `PostParams`.
struct PostParams {
    // x: exposure (EV), y: contrast, z: saturation, w: gamma. Only x is read by
    // the composite: the rest reach it baked into the grading LUT (#1440).
    color: vec4<f32>,
    // x: bloom_intensity, y: bloom_threshold, z: tonemap index, w: bloom_enabled
    bloom: vec4<f32>,
    // x: blur direction (0=horizontal,1=vertical), y: texel_x, z: texel_y, w: unused
    misc: vec4<f32>,
    // x: ssr_mode (0 off, 1 cubemap, 2 screen-space), yz: unused,
    // w: auto exposure (1: the composite applies the adapted EV)
    flags: vec4<f32>,
    // current inverse view-projection (reconstruct world pos from depth)
    inv_view_proj: mat4x4<f32>,
    // current view-projection (SSR ray marching to clip space)
    view_proj: mat4x4<f32>,
    // xyz: camera world position, w: the sim's game time in seconds (#398) — the
    // same clock as `CameraUniforms.time`, 0 in edit mode and previews
    camera_pos: vec4<f32>,
    // The lens effects (#1439, `postfx/lens.wgsl`). x: vignette intensity (0 off),
    // y: vignette smoothness, zw: vignette centre uv
    vignette: vec4<f32>,
    // rgb: the vignette's colour
    vignette_color: vec4<f32>,
    // x: chromatic aberration intensity (0 off), y: film grain intensity (0 off),
    // z: film grain response
    lens: vec4<f32>,
};
