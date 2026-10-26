// engine/shaders/postfx/effects/dummy.wgsl — the test-only dummy effect's composite
// function (`render::postfx::effects::dummy`, #1473), the worked example of a
// composite texture: `probe` is the dummy's own texture, which it never provides, so
// the registry binds black and the add changes nothing. No shipped build lists the
// dummy, so no shipped composite imports this module.

#define_import_path postfx::effects::dummy

#import postfx::frame::{Frag, s_color}

fn hdr(c: vec3<f32>, f: Frag, probe: texture_2d<f32>) -> vec3<f32> {
    return c + textureSampleLevel(probe, s_color, f.uv, 0.0).rgb;
}
