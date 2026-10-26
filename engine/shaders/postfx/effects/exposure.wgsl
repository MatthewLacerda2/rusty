// engine/shaders/postfx/effects/exposure.wgsl — the exposure (#1444), the one colour
// knob outside the LUT: a pre-grade HDR scale, the volume's fixed EV or, in Automatic,
// the adapted one the meter (`postfx/exposure.wgsl`) left. Slot 0: x the fixed EV, y
// `1` in Automatic.

#define_import_path postfx::effects::exposure

#import postfx::frame::{Frag, adapted}

fn hdr(c: vec3<f32>, f: Frag, p: vec4<f32>) -> vec3<f32> {
    return c * exp2(select(p.x, adapted.x, p.y > 0.5));
}
