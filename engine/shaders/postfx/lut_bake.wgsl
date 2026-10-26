// engine/shaders/postfx/lut_bake.wgsl — bakes the grading LUT (#1440): one
// invocation per texel of the 32³ table, each storing `grade` of the HDR colour the
// texel's LogC code decodes to. Runs only when a grading knob or the tonemap
// changes (`GradeLut::prepare`).

#import postfx::grade::{Grade, LUT_SIZE, grade_texel}

@group(0) @binding(0) var<uniform> g: Grade;
@group(0) @binding(1) var lut: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(4, 4, 4)
fn cs_bake(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id >= vec3<u32>(u32(LUT_SIZE)))) {
        return;
    }
    textureStore(lut, id, vec4<f32>(grade_texel(id, g), 1.0));
}
