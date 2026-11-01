//! The CPU's procedural sky is the shader's (#1675): `Skybox` mode projects
//! `scene::procedural_sky_color` into the ambient, so a retune of
//! `common::procedural_sky` that left it behind would light the scene with a sky it
//! no longer draws. Clouds off (`cloud_opacity = 0`): the CPU hashes its noise
//! differently on purpose (`skybox::procedural`), so only the gradient is the same
//! function term for term.

use glam::Vec3;

use super::ProceduralSkyUniform;
use crate::scene::{procedural_sky_color, SkySettings};

const AMBIENT: Vec3 = Vec3::new(0.3, 0.5, 0.9);

/// Per invocation (128): a unit direction spread over the sphere, then the sky
/// along it.
const PROBE: &str = "
#import common::{ProceduralSky, procedural_sky}
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.y * 64u + id.x;
    let y = 1.0 - f32(i) / 127.0 * 2.0;
    let r = sqrt(max(1.0 - y * y, 0.0));
    let phi = f32(i) * 2.39996;
    let dir = vec3<f32>(r * cos(phi), y, r * sin(phi));
    out[i] = vec4<f32>(procedural_sky(SKY, AMBIENT, dir), 0.0);
}
";

fn directions() -> impl Iterator<Item = Vec3> {
    (0..128).map(|i| {
        let y = 1.0 - i as f32 / 127.0 * 2.0;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let phi = i as f32 * 2.39996;
        Vec3::new(r * phi.cos(), y, r * phi.sin())
    })
}

/// The sky along each direction, as the GPU draws `sky`.
fn gpu_sky(r: &crate::render::Renderer, sky: &SkySettings) -> Vec<Vec3> {
    let u = ProceduralSkyUniform::from_settings(sky);
    let v = |c: [f32; 4]| format!("vec4<f32>({:?}, {:?}, {:?}, {:?})", c[0], c[1], c[2], c[3]);
    let consts = format!(
        "const SKY = ProceduralSky({}, {}, {}, {});\nconst AMBIENT = vec3<f32>({:?}, {:?}, {:?});\n",
        v(u.zenith),
        v(u.horizon),
        v(u.ground),
        v(u.clouds),
        AMBIENT.x,
        AMBIENT.y,
        AMBIENT.z
    );
    let base = crate::shadergen::engine_shader_dir();
    let mut composer = crate::shadergen::compose::composer_with_common(base).unwrap();
    let source = format!("{PROBE}{consts}");
    let module = crate::shadergen::compose::compose(&mut composer, &source, "sky_probe").unwrap();
    let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
    let out = crate::render::test_gpu::run_probe(&r.device, &r.queue, source);
    out.iter()
        .take(128)
        .map(|p| Vec3::from_slice(&p[..3]))
        .collect()
}

#[test]
fn gpu_the_cpus_procedural_sky_is_the_shaders() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let derived = SkySettings {
        cloud_opacity: 0.0,
        ..SkySettings::default()
    };
    let set = SkySettings {
        zenith: Some(Vec3::new(0.1, 0.3, 0.8)),
        horizon: Some(Vec3::new(0.9, 0.6, 0.4)),
        ground: Some(Vec3::new(0.2, 0.15, 0.1)),
        ..derived
    };
    for sky in [derived, set] {
        let gpu = gpu_sky(&r, &sky);
        let gap = directions()
            .zip(&gpu)
            .map(|(d, g)| {
                (procedural_sky_color(&sky, AMBIENT, d) - *g)
                    .abs()
                    .max_element()
            })
            .fold(0.0, f32::max);
        assert!(gap <= 1e-4, "{sky:?}: CPU and GPU skies differ by {gap}");
    }
}
