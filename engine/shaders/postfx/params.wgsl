// engine/shaders/postfx/params.wgsl — the post-process chain's frame uniform, its own
// module (#1439): every post pass binds it, built-in (the composite, `postfx/frame.wgsl`)
// and authored (the `Shader.Bake` postfx scaffold, which reads the game time from it,
// #398). It holds only what every effect shares: each effect's own numbers live in its
// slots of the composite's effect array (`render::postfx::effects`, #1467).

#define_import_path postfx::params

// The post-process chain's frame uniform (group 0, binding 0 of every post pass,
// built-in and authored). Mirrors the Rust `PostParams`.
struct PostParams {
    // current inverse view-projection (reconstruct world pos from depth)
    inv_view_proj: mat4x4<f32>,
    // current view-projection (SSR ray marching to clip space)
    view_proj: mat4x4<f32>,
    // xyz: camera world position, w: the sim's game time in seconds (#398) — the
    // same clock as `CameraUniforms.time`, 0 in edit mode and previews
    camera_pos: vec4<f32>,
};
