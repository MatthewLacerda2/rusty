// engine/shaders/postfx/effects/lens_flare.wgsl — the lens flare's add, in the
// composite (#1443, #1473): the flare `postfx/lens_flare.wgsl` drew before the
// composite, at a quarter of the view's size or less, upsampled onto the HDR image
// after bloom's add, as URP's uber pass adds it with bloom. Added here rather than
// onto the scene, SSR doesn't reflect it and the chromatic aberration doesn't split
// it. `image` is the flare's composite texture: black while the view doesn't flare,
// so the add is then exactly nothing. Slot 0: w `1` while the view flares; off,
// the add is skipped without a fetch.

#define_import_path postfx::effects::lens_flare

#import postfx::frame::{Frag, s_color}

fn hdr(c: vec3<f32>, f: Frag, image: texture_2d<f32>, p: vec4<f32>) -> vec3<f32> {
    if (p.w < 0.5) {
        return c;
    }
    return c + textureSampleLevel(image, s_color, f.uv, 0.0).rgb;
}
