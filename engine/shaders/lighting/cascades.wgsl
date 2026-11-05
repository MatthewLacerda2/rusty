// engine/shaders/lighting/cascades.wgsl — the sun's cascaded shadow maps as the
// shaders read them (#435): the forward pass (`forward::bindings::shadow`) and the
// volumetric fog's froxels (#1703) bind the same uniform.

#define_import_path lighting::cascades

// One light volume per slice of the view, written by
// `render::passes::shadows::CascadeUniform`.
struct ShadowCascades {
    light_space: array<mat4x4<f32>, 4>,
    splits: vec4<f32>,       // view depth where each cascade ends
    texels: vec4<f32>,       // world size of one texel, per cascade
    depth_ranges: vec4<f32>, // world depth each cascade's [0, 1] spans
    view_pos: vec4<f32>,
    view_dir: vec4<f32>,
    params: vec4<f32>,       // x count, y shadow distance, z blend fraction
};
