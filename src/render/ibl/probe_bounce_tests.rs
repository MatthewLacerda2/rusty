//! Tests for the multi-bounce probe solve (#285). Sibling of `probe_bounce.rs`;
//! shares its 300-line source cap.
//!
//! The convergence logic (`field_delta` / `pass_converged`) is pure and always runs,
//! so the early-out decision is covered on any host. The GPU multi-bounce bakes skip
//! gracefully when no adapter is present — same contract as `probe_bake_tests.rs`.

use glam::Vec3;

use crate::render::ibl::probe_bounce::{field_delta, pass_converged};
use crate::render::MAX_BOUNCES;
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

const RES: u32 = 32;

// --- Pure convergence logic (no GPU) ------------------------------------------------

/// A non-zero per-coefficient difference is reported as the max absolute delta.
#[test]
fn field_delta_is_max_abs_coefficient() {
    let mut a = Sh9::zero();
    let mut b = Sh9::zero();
    a.coeffs[0] = [0.0, 0.0, 0.0];
    b.coeffs[0] = [0.2, 0.0, 0.0];
    b.coeffs[4] = [0.0, -0.5, 0.0]; // larger magnitude, on a different coeff/channel
    assert!((field_delta(&[a], &[b]) - 0.5).abs() < 1.0e-6);
}

/// Identical fields have zero delta and count as converged for any positive epsilon.
#[test]
fn identical_fields_converge() {
    let f = vec![Sh9::zero(); 3];
    assert_eq!(field_delta(&f, &f), 0.0);
    assert!(pass_converged(&f, &f, 1.0e-6));
}

/// `pass_converged` is exactly the delta-below-epsilon predicate: a small change
/// stops, a large one continues.
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

// --- GPU multi-bounce bakes (graceful-skip) -----------------------------------------

fn box_mesh() -> crate::scene::MeshComponent {
    let (vertices, indices) = crate::render::gpu::mesh::generate_box(1.0, 1.0, 1.0);
    crate::scene::MeshComponent {
        primitive_type: "Box".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: crate::scene::DirtyFlag::new(true),
    }
}

/// A static box at `pos` (scale `scale`) with the given base colour and emissive.
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
        crate::components::MaterialAsset {
            base_color,
            emissive,
            ..crate::components::MaterialAsset::default()
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
    scene.world.set_material(
        id,
        Some(crate::components::MaterialComponent { material: mat_name }),
    );
}

/// A closed-ish room: a bright emissive light source plus a strongly-coloured red
/// wall that bounces, and a neutral grey floor/ceiling that re-radiate the red. With
/// real bounces the red bleed onto the field grows past the first capture.
fn bleed_room(probe: Vec3) -> Scene {
    let mut scene = Scene::new();
    // Emissive ceiling panel (the direct light).
    add_box(
        &mut scene,
        "Light",
        Vec3::new(0.0, 4.0, 0.0),
        Vec3::new(8.0, 0.5, 8.0),
        [0.0, 0.0, 0.0],
        [6.0, 6.0, 6.0],
    );
    // Strong red reflective wall on +X.
    add_box(
        &mut scene,
        "RedWall",
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(0.5, 8.0, 8.0),
        [0.9, 0.05, 0.05],
        [0.0, 0.0, 0.0],
    );
    // Neutral floor that catches and re-radiates the bounced red.
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

/// Multi-bounce signal: the converged field carries MORE indirect red than a single
/// direct-only capture (`bake_probe`) — bounces ≥2 fed the red wall's reflection back
/// into the scene. The issue's headline acceptance.
#[test]
fn multibounce_increases_color_bleed() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return; // No GPU/software adapter — skip, same contract as the screenshots.
    };
    let probe = Vec3::new(0.0, 0.0, 0.0);

    // Bounce 1 only (the old single-capture result).
    let b1 = renderer.bake_probe(&bleed_room(probe), probe, RES);

    // Full multi-bounce solve.
    let mut scene = bleed_room(probe);
    let report = renderer.bake_probes_multibounce(&mut scene, RES);
    let converged = scene.probes.probes[0].sh;

    assert!(report.bounces >= 2, "expected ≥2 bounces, got {report:?}");
    // Toward the red wall (+X), the converged irradiance is redder than bounce 1.
    let toward1 = b1.eval(Vec3::X);
    let toward_n = converged.eval(Vec3::X);
    assert!(
        toward_n.x > toward1.x + 1.0e-4,
        "expected more red after bounces: {toward_n} vs {toward1}"
    );
}

/// Early-out: a near-black, low-albedo, unlit scene has almost no light to bounce, so
/// the field stops changing well before the cap — `converged` fires and `bounces` is
/// short of [`MAX_BOUNCES`].
#[test]
fn early_out_on_dark_scene() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    // A single very dark, non-emissive box — no source to bounce.
    add_box(
        &mut scene,
        "DarkBox",
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(1.0, 4.0, 4.0),
        [0.02, 0.02, 0.02],
        [0.0, 0.0, 0.0],
    );
    scene.probes.add_probe(Vec3::ZERO);

    let report = renderer.bake_probes_multibounce(&mut scene, RES);
    assert!(report.converged, "dark scene should converge: {report:?}");
    assert!(
        report.bounces < MAX_BOUNCES,
        "dark scene should not burn all bounces: {report:?}"
    );
}

/// Determinism: the same scene bakes byte-identical SH across two independent runs.
#[test]
fn multibounce_is_deterministic() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let probe = Vec3::new(0.0, 0.0, 0.0);

    let mut a = bleed_room(probe);
    let mut b = bleed_room(probe);
    let ra = renderer.bake_probes_multibounce(&mut a, RES);
    let rb = renderer.bake_probes_multibounce(&mut b, RES);

    assert_eq!(ra, rb, "bounce reports diverged: {ra:?} vs {rb:?}");
    assert_eq!(
        a.probes.probes[0].sh, b.probes.probes[0].sh,
        "multi-bounce SH not deterministic across runs"
    );
}
