// engine/shaders/postfx/effects/white_balance.wgsl — White Balance (#1440), the first
// step of the grading bake, on the scene-linear colour. Slot 0: xyz the LMS
// coefficients that move D65 to the chosen white point, w `1` when it runs.

#define_import_path postfx::effects::white_balance

// Unity's white balance: scale the colour in LMS cone space by `coeffs`. `v * M` with
// M's columns the rows of Unity's matrices is its `mul(M, v)`.
fn balance(c: vec3<f32>, coeffs: vec3<f32>) -> vec3<f32> {
    let to_lms = mat3x3<f32>(
        vec3<f32>(3.90405e-1, 5.49941e-1, 8.92632e-3),
        vec3<f32>(7.08416e-2, 9.63172e-1, 1.35775e-3),
        vec3<f32>(2.31082e-2, 1.28021e-1, 9.36245e-1),
    );
    let from_lms = mat3x3<f32>(
        vec3<f32>(2.85847e+0, -1.62879e+0, -2.48910e-2),
        vec3<f32>(-2.10182e-1, 1.15820e+0, 3.24281e-4),
        vec3<f32>(-4.18120e-2, -1.18169e-1, 1.06867e+0),
    );
    return ((c * to_lms) * coeffs) * from_lms;
}

fn grade(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.w > 0.5) {
        out = balance(out, p.xyz);
    }
    return out;
}
