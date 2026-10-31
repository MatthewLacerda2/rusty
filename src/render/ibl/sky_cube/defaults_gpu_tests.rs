//! The default procedural sky against today's (#1620): `procedural_sky` fed the
//! Rust defaults (`SkySettings::default`, so `cloud_shift(0.5)`, opacity 0.6 and the
//! derived colours) must draw what it drew before the settings existed. Both run in
//! one compute probe on one adapter, so the clouds' `sin` hash, which drivers round
//! differently (Metal vs lavapipe), cancels out.

use glam::Vec3;

use crate::render::gpu::uniforms::ProceduralSkyUniform;
use crate::scene::SkySettings;

/// `procedural_sky` as `main` drew it at 01cfc8b, before the settings: frozen here as
/// the reference, never edited to follow a change.
const TODAY: &str = "
fn today_sky(sky_color: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let ground_color = sky_color * 0.25;
    let horizon_color = mix(sky_color, vec3<f32>(1.0), 0.6);
    let up = clamp(dir.y, 0.0, 1.0);
    let down = clamp(-dir.y, 0.0, 1.0);
    var color = mix(horizon_color, sky_color, pow(up, 0.5));
    color = mix(color, ground_color, pow(down, 0.35));
    if (dir.y > 0.0) {
        let dome = dir.xz / max(dir.y, 0.15);
        let coverage = smoothstep(0.45, 0.80, sky_fbm(dome * 1.6));
        let band = smoothstep(0.05, 0.35, up);
        let cloud_color = mix(sky_color, vec3<f32>(1.0), 0.85);
        color = mix(color, cloud_color, coverage * band * 0.6);
    }
    return color;
}
";

/// What the probe reads, imported ahead of the `SKY` it declares.
const IMPORTS: &str = "
#import common::{ProceduralSky, procedural_sky}
#import sky::noise::sky_fbm
";

/// Per invocation (128): a direction spread over the sphere, biased up where the
/// clouds are, and one of four ambient colours; writes the sky under `SKY` (the
/// settings under test), then today's.
const PROBE: &str = "
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.y * 64u + id.x;
    let y = 1.0 - f32(i) / 127.0 * 1.3;
    let r = sqrt(max(1.0 - y * y, 0.0));
    let phi = f32(i) * 2.39996;
    let dir = vec3<f32>(r * cos(phi), y, r * sin(phi));
    var ambients = array<vec3<f32>, 4>(
        vec3<f32>(0.0), vec3<f32>(1.0), vec3<f32>(0.4, 0.5, 0.6), vec3<f32>(0.9, 0.3, 0.1)
    );
    let ambient = ambients[i % 4u];
    out[2u * i] = vec4<f32>(procedural_sky(SKY, ambient, dir), 0.0);
    out[2u * i + 1u] = vec4<f32>(today_sky(ambient, dir), 0.0);
}
";

/// `settings` as a WGSL `ProceduralSky` literal, exactly as the uniform carries it.
fn literal(settings: &SkySettings) -> String {
    let u = ProceduralSkyUniform::from_settings(settings);
    let v = |c: [f32; 4]| format!("vec4<f32>({:?}, {:?}, {:?}, {:?})", c[0], c[1], c[2], c[3]);
    let fields = [u.zenith, u.horizon, u.ground, u.clouds].map(v).join(", ");
    format!("ProceduralSky({fields})")
}

/// The largest gap, over every sample, between the sky under `settings` and today's.
fn gap(r: &crate::render::Renderer, settings: &SkySettings) -> f32 {
    let base = crate::shadergen::engine_shader_dir();
    let mut composer = crate::shadergen::compose::composer_with_common(base).unwrap();
    let source = format!(
        "{IMPORTS}const SKY = {};\n{TODAY}{PROBE}",
        literal(settings)
    );
    let module = crate::shadergen::compose::compose(&mut composer, &source, "sky_probe").unwrap();
    let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
    let out = crate::render::test_gpu::run_probe(&r.device, &r.queue, source);
    out.chunks_exact(2)
        .map(|p| {
            (Vec3::from_slice(&p[0][..3]) - Vec3::from_slice(&p[1][..3]))
                .abs()
                .max_element()
        })
        .fold(0.0, f32::max)
}

/// A named nudge to one setting.
type Change = (&'static str, fn(&mut SkySettings));

#[test]
fn gpu_the_default_sky_is_todays() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    // Float noise only: the expressions match term for term.
    let tolerance = 1e-5;
    let default = SkySettings::default();
    let got = gap(&r, &default);
    assert!(
        got <= tolerance,
        "the default sky drifts {got} from today's"
    );

    // The probe still sees a small change to any default.
    let changes: [Change; 3] = [
        ("coverage", |s| s.cloud_coverage += 0.02),
        ("opacity", |s| s.cloud_opacity += 0.02),
        ("ground", |s| s.ground = Some(Vec3::ZERO)),
    ];
    for (name, change) in changes {
        let mut sky = default;
        change(&mut sky);
        let moved = gap(&r, &sky);
        assert!(
            moved > 100.0 * tolerance,
            "{name} moved the sky only {moved}"
        );
    }
}
