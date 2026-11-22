use glam::Vec3;

use crate::ibl::probe::bounce::{field_delta, pass_converged};
use crate::MAX_BOUNCES;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::Scene;

const RES: u32 = 32;

#[test]
fn field_delta_is_max_abs_coefficient() {
    let mut a = Sh9::zero();
    let mut b = Sh9::zero();
    a.coeffs[0] = [0.0, 0.0, 0.0];
    b.coeffs[0] = [0.2, 0.0, 0.0];
    b.coeffs[4] = [0.0, -0.5, 0.0]; // larger magnitude, on a different coeff/channel
    assert!((field_delta(&[a], &[b]) - 0.5).abs() < 1.0e-6);
}

#[test]
fn identical_fields_converge() {
    let f = vec![Sh9::zero(); 3];
    assert_eq!(field_delta(&f, &f), 0.0);
    assert!(pass_converged(&f, &f, 1.0e-6));
}

#[test]
fn pass_converged_thresholds_on_epsilon() {
    let mut a = Sh9::zero();
    let mut tiny = Sh9::zero();
    let mut big = Sh9::zero();
    a.coeffs[0] = [1.0, 1.0, 1.0];
    tiny.coeffs[0] = [1.0 + 1.0e-4, 1.0, 1.0]; // delta 1e-4
    big.coeffs[0] = [1.5, 1.0, 1.0]; // delta 0.5
    let eps = 1.0e-3;
    assert!(pass_converged(&[a], &[tiny], eps), "tiny delta should stop");
    assert!(
        !pass_converged(&[a], &[big], eps),
        "big delta should continue"
    );
}

fn box_mesh() -> rusty_scene::MeshComponent {
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    rusty_scene::MeshComponent::new("Box", vertices, indices)
}

fn add_box(
    scene: &mut Scene,
    name: &str,
    pos: Vec3,
    scale: Vec3,
    base_color: [f32; 3],
    emissive: [f32; 3],
) {
    let mat_name = format!("{name}_mat");
    scene.materials.insert(
        mat_name.clone(),
        rusty_components::MaterialAsset {
            base_color,
            emissive,
            ..rusty_components::MaterialAsset::default()
        },
    );
    let id = scene.add_entity(name.to_string());
    scene.world.set_mesh(id, Some(box_mesh()));
    scene.world.set_static(id, true);
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = pos;
        t.scale = scale;
        t.rotation = glam::Quat::IDENTITY;
    }
    scene
        .world
        .set_material(id, Some(rusty_components::MaterialComponent::new(mat_name)));
}

fn bleed_room(probe: Vec3) -> Scene {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        "Light",
        Vec3::new(0.0, 4.0, 0.0),
        Vec3::new(8.0, 0.5, 8.0),
        [0.0, 0.0, 0.0],
        [6.0, 6.0, 6.0],
    );
    add_box(
        &mut scene,
        "RedWall",
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(0.5, 8.0, 8.0),
        [0.9, 0.05, 0.05],
        [0.0, 0.0, 0.0],
    );
    add_box(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -4.0, 0.0),
        Vec3::new(8.0, 0.5, 8.0),
        [0.8, 0.8, 0.8],
        [0.0, 0.0, 0.0],
    );
    scene.probes.add_probe(probe);
    scene
}

#[test]
fn gpu_multibounce_increases_color_bleed() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let probe = Vec3::new(0.0, 0.0, 0.0);

    let b1 = renderer.bake_probe(&bleed_room(probe), probe, RES);

    let mut scene = bleed_room(probe);
    let report =
        renderer.bake_probes_multibounce(&mut scene, RES, rusty_lighting::lightmap::probe_direct);
    let converged = scene.probes.probes()[0].sh;

    assert!(report.bounces >= 2, "expected ≥2 bounces, got {report:?}");
    let toward1 = b1.eval(Vec3::X);
    let toward_n = converged.eval(Vec3::X);
    assert!(
        toward_n.x > toward1.x + 1.0e-4,
        "expected more red after bounces: {toward_n} vs {toward1}"
    );
}

#[test]
fn gpu_early_out_on_dark_scene() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        "DarkBox",
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(1.0, 4.0, 4.0),
        [0.02, 0.02, 0.02],
        [0.0, 0.0, 0.0],
    );
    scene.probes.add_probe(Vec3::ZERO);

    let report =
        renderer.bake_probes_multibounce(&mut scene, RES, rusty_lighting::lightmap::probe_direct);
    assert!(report.converged, "dark scene should converge: {report:?}");
    assert!(
        report.bounces < MAX_BOUNCES,
        "dark scene should not burn all bounces: {report:?}"
    );
}

#[test]
fn gpu_multibounce_is_deterministic() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let probe = Vec3::new(0.0, 0.0, 0.0);

    let mut a = bleed_room(probe);
    let mut b = bleed_room(probe);
    let ra = renderer.bake_probes_multibounce(&mut a, RES, rusty_lighting::lightmap::probe_direct);
    let rb = renderer.bake_probes_multibounce(&mut b, RES, rusty_lighting::lightmap::probe_direct);

    assert_eq!(ra, rb, "bounce reports diverged: {ra:?} vs {rb:?}");
    assert_eq!(
        a.probes.probes()[0].sh,
        b.probes.probes()[0].sh,
        "multi-bounce SH not deterministic across runs"
    );
}
