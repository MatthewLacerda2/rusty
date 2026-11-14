//! The SSR surface targets' alpha packing (#1736), run as written on the GPU:
//! `ssr_surface.wgsl`'s `pack_lane` gives an exact normal `f16` (so a render target
//! stores it unchanged), and `unpack_lane` returns the component, quantised, and
//! the digit, over the whole range: both ends, the top level with the top digit,
//! an exact 0, and the clear texel.

use crate::test_gpu::{headless_or_skip, run_probe};

/// Invocations 0..=126 pack component `-1 + 2i/126` (63: exactly 0, 126: 1, the
/// top level) with digit `i % 15` (126: 14, the top digit); 127 unpacks the clear
/// texel. Each writes (component, digit, 1 if the packed value is an exact normal
/// `f16`, the packed value).
const PROBE: &str = "
#import lighting::ssr_surface::{pack_lane, unpack_lane}
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.y * 64u + id.x;
    if (i == 127u) {
        out[i] = vec4<f32>(unpack_lane(0.0), 1.0, 0.0);
        return;
    }
    let e = -1.0 + 2.0 * f32(i) / 126.0;
    let digit = select(i % 15u, 14u, i == 126u);
    let a = pack_lane(e, digit);
    let bits = bitcast<u32>(a);
    let exponent = (bits >> 23u) & 0xffu;
    let exact = (bits & 0x1fffu) == 0u && exponent >= 113u && exponent <= 142u;
    out[i] = vec4<f32>(unpack_lane(a), select(0.0, 1.0, exact), a);
}
";

/// What `unpack_lane` should return for component `e`: its level of 2046, back.
fn quantised(e: f32) -> f32 {
    ((e * 0.5 + 0.5) * 2046.0).round() / 2046.0 * 2.0 - 1.0
}

#[test]
fn gpu_the_packed_alpha_is_an_exact_f16_and_unpacks_to_its_parts() {
    let Some(r) = headless_or_skip(8, 8) else {
        return;
    };
    let base = rusty_shadergen::engine_shader_dir();
    let mut composer = rusty_shadergen::compose::composer_with_common(base).unwrap();
    let module = rusty_shadergen::compose::compose(&mut composer, PROBE, "pack_probe").unwrap();
    let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
    let out = run_probe(&r.device, &r.queue, source);
    for (i, [e, digit, exact, a]) in out.iter().copied().take(127).enumerate() {
        let want = quantised(-1.0 + 2.0 * i as f32 / 126.0);
        let want_digit = if i == 126 { 14 } else { i % 15 };
        assert_eq!(exact, 1.0, "case {i}: {a} is not an exact normal f16");
        assert!(
            (e - want).abs() < 1e-6,
            "case {i}: component {e}, want {want}"
        );
        assert_eq!(digit, want_digit as f32, "case {i}: digit");
    }
    assert_eq!(out[63][0], 0.0, "a 0 component decodes to exactly 0");
    assert!(
        out[126][3] <= 65504.0,
        "the top level with digit 14 fits an f16"
    );
    let clear = out[127];
    assert!(
        clear[0].is_finite() && clear[1] == 0.0,
        "the clear texel: {clear:?}"
    );
}
