// engine/shaders/postfx/effects/lift_gamma_gain.wgsl — Lift / Gamma / Gain (#1440), on
// the scene-linear colour after white balance. Slot 0: xyz lift (added after gain);
// slot 1: xyz `1 / gamma`, the gamma wheel's per-channel power; slot 2: xyz gain.

#define_import_path postfx::effects::lift_gamma_gain

fn grade(c_in: vec3<f32>, lift: vec4<f32>, inv_gamma: vec4<f32>, gain: vec4<f32>) -> vec3<f32> {
    var c = c_in;
    c = c * gain.xyz + lift.xyz;
    c = sign(c) * pow(abs(c), inv_gamma.xyz);
    // Into what the scene target holds (0 to f16's largest): a negative lift would
    // reach the tonemappers, which turn it grey (ACES) or tinted (Reinhard), and a
    // gamma near 0 makes highlights infinite, which they turn into NaN.
    return clamp(c, vec3<f32>(0.0), vec3<f32>(65504.0));
}
