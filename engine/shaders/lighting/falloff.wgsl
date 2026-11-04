// engine/shaders/lighting/falloff.wgsl — how a point, spot or Area light fades
// (#1699): `common::local_light_radiance` is its caller, and the CPU's twins are
// `components::falloff` / `components::spot_cone`.

#define_import_path lighting::falloff

// URP's squared-distance floor: a surface touching a lamp stays finite.
const FALLOFF_EPSILON_SQ: f32 = 0.0001;

// How a local light fades at distance `d` within `range` (#1699): Unity URP's and
// Frostbite's windowed inverse-square, saturate(1 - (d/r)^4)^2 / max(d^2, eps^2),
// exactly zero at and past `range`. The CPU's twin is `components::falloff`, held
// equal by `render::clusters::falloff_gpu_tests`.
fn light_falloff(d: f32, range: f32) -> f32 {
    if (d >= range) {
        return 0.0;
    }
    let ratio = d / range;
    let r2 = ratio * ratio;
    let window = clamp(1.0 - r2 * r2, 0.0, 1.0);
    return window * window / max(d * d, FALLOFF_EPSILON_SQ);
}

// A spotlight's cone at `cos_theta` off its axis: URP's squared ramp between the
// outer and inner cones. The CPU's twin is `components::spot_cone`.
fn spot_cone(cos_theta: f32, cos_inner: f32, cos_outer: f32) -> f32 {
    let t = clamp((cos_theta - cos_outer) / max(cos_inner - cos_outer, 1e-4), 0.0, 1.0);
    return t * t;
}
