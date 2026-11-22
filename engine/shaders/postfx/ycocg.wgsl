// The colour spaces TAA's resolve works in (`taa.wgsl`): YCoCg, where its neighbourhood box is
// built and its history clamped, and URP's perceptual luma, what its flicker hold measures
// (`taa_flicker.wgsl`).

#define_import_path postfx::ycocg

fn from_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(c, vec3<f32>(0.25, 0.5, 0.25)),
        dot(c, vec3<f32>(0.5, 0.0, -0.5)),
        dot(c, vec3<f32>(-0.25, 0.5, -0.25)),
    );
}

fn to_rgb(c: vec3<f32>) -> vec3<f32> {
    let t = c.x - c.z;
    return vec3<f32>(t + c.y, c.x + c.z, t - c.y);
}

// A colour's luma (YCoCg's `y`, linear) in URP's perceptual space, `y / (1 + y)`,
// in `[0, 1)`: as the resolve's blend weighs colours, so a bright pixel counts as
// the eye sees it.
fn perceptual(y: f32) -> f32 {
    let l = max(y, 0.0);
    return l / (1.0 + l);
}
