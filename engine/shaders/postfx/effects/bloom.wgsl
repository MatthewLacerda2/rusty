// engine/shaders/postfx/effects/bloom.wgsl — bloom's add, in the composite: the
// blurred bright pass (`postfx/bloom.wgsl`, run before the composite) over the HDR
// image. Slot 0: x intensity, w `1` while bloom runs.

#define_import_path postfx::effects::bloom

#import postfx::frame::{Frag, t_bloom, s_color}

fn hdr(c: vec3<f32>, f: Frag, p: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.w > 0.5) {
        out += textureSample(t_bloom, s_color, f.uv).rgb * p.x;
    }
    return out;
}
