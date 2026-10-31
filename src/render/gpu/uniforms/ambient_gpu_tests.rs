//! The ambient's one evaluation (#1647), run on the GPU: `lighting::ambient::ambient_radiance`
//! fed the uniform a scene uploads must draw, in `Color` mode, exactly what the
//! forward pass drew before the gradient, and in either mode what the CPU's
//! `AmbientRadiance::at` (lightmaps, probes) computes.

use glam::Vec3;

use crate::render::draw::lighting::scene_ambient;
use crate::scene::{AmbientMode, Scene};

/// `flat_ambient` as `main` drew it at fe8afe0, before the gradient: frozen here as
/// the reference, never edited to follow a change.
const TODAY: &str = "
fn today_ambient(color: vec3<f32>, intensity: f32, N: vec3<f32>) -> vec3<f32> {
    let sky_color = color;
    let ground_color = sky_color * 0.25;
    return mix(ground_color, sky_color, N.y * 0.5 + 0.5) * intensity;
}
";

/// Per invocation (128): a unit direction spread over the sphere, then the ambient
/// under `AMBIENT` along it, and today's.
const PROBE: &str = "
#import lighting::ambient::{AmbientLight, ambient_radiance}
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.y * 64u + id.x;
    let y = 1.0 - f32(i) / 127.0 * 2.0;
    let r = sqrt(max(1.0 - y * y, 0.0));
    let phi = f32(i) * 2.39996;
    let dir = vec3<f32>(r * cos(phi), y, r * sin(phi));
    out[2u * i] = vec4<f32>(ambient_radiance(AMBIENT, dir), 0.0);
    out[2u * i + 1u] = vec4<f32>(today_ambient(AMBIENT.color, AMBIENT.intensity, dir), 0.0);
}
";

/// The same 128 directions on the CPU.
fn directions() -> impl Iterator<Item = Vec3> {
    (0..128).map(|i| {
        let y = 1.0 - i as f32 / 127.0 * 2.0;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let phi = i as f32 * 2.39996;
        Vec3::new(r * phi.cos(), y, r * phi.sin())
    })
}

/// `scene`'s ambient uniform as a WGSL `AmbientLight` literal.
fn literal(scene: &Scene) -> String {
    let u = scene_ambient(scene);
    let v = |c: [f32; 3]| format!("vec3<f32>({:?}, {:?}, {:?})", c[0], c[1], c[2]);
    format!(
        "AmbientLight({}, {:?}, {}, {:?}, {}, 0.0, {}, 0.0)",
        v(u.color),
        u.intensity,
        v(u.gradient_sky),
        u.gradient,
        v(u.gradient_equator),
        v(u.gradient_ground)
    )
}

/// `(new, today)` per direction, as the GPU computes them for `scene`.
fn probe(r: &crate::render::Renderer, scene: &Scene) -> Vec<(Vec3, Vec3)> {
    let base = crate::shadergen::engine_shader_dir();
    let mut composer = crate::shadergen::compose::composer_with_common(base).unwrap();
    let source = format!("const AMBIENT = {};\n{TODAY}{PROBE}", literal(scene));
    let module =
        crate::shadergen::compose::compose(&mut composer, &source, "ambient_probe").unwrap();
    let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module));
    let out = crate::render::test_gpu::run_probe(&r.device, &r.queue, source);
    out.chunks_exact(2)
        .take(128)
        .map(|p| (Vec3::from_slice(&p[0][..3]), Vec3::from_slice(&p[1][..3])))
        .collect()
}

/// The largest gap between the GPU's ambient and the CPU's, over every direction.
fn cpu_gap(scene: &Scene, gpu: &[(Vec3, Vec3)]) -> f32 {
    let cpu = scene.drawn_ambient().radiance();
    directions()
        .zip(gpu)
        .map(|(dir, (got, _))| (cpu.at(dir) - *got).abs().max_element())
        .fold(0.0, f32::max)
}

#[test]
fn gpu_the_ambient_is_todays_in_color_mode_and_the_cpus_in_both() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    // The default scene, and a coloured, brighter one: the Color path is today's
    // expression term for term, so the two agree to the last bit (a hair of slack
    // only for a driver that contracts the two copies differently).
    let mut bright = Scene::new();
    bright.ambient_color = Vec3::new(0.9, 0.3, 0.1);
    bright.ambient_intensity = 2.5;
    for scene in [Scene::new(), bright] {
        let gpu = probe(&r, &scene);
        let drift = gpu.iter().map(|(n, t)| (*n - *t).abs().max_element());
        let drift = drift.fold(0.0, f32::max);
        assert!(drift <= 1e-6, "Color drifts {drift} from today");
        assert!(cpu_gap(&scene, &gpu) <= 1e-5, "Color: CPU and GPU disagree");
    }
    // A warm ground under a blue sky.
    let mut gradient = Scene::new();
    gradient.ambient.mode = AmbientMode::Gradient;
    gradient.ambient.sky = Vec3::new(0.2, 0.4, 1.0);
    gradient.ambient.equator = Vec3::new(0.5, 0.5, 0.5);
    gradient.ambient.ground = Vec3::new(0.8, 0.4, 0.1);
    gradient.ambient_intensity = 1.5;
    let gpu = probe(&r, &gradient);
    let gap = cpu_gap(&gradient, &gpu);
    assert!(gap <= 1e-5, "Gradient: CPU and GPU disagree by {gap}");
    let moved = gpu.iter().map(|(n, t)| (*n - *t).abs().max_element());
    assert!(
        moved.fold(0.0, f32::max) > 0.1,
        "the probe sees the gradient"
    );
}
