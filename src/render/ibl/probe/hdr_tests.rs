//! Light-probe captures in HDR (#1696): with nothing clamping the bounces, the
//! solve still converges. Radiance above 1 and a texel past the half float's
//! range: `hdr_finite_tests`.

use glam::{Quat, Vec3};

use crate::components::{MaterialAsset, MaterialComponent};
use crate::render::project_cubemap;
use crate::scene::lighting::sh::Sh9;
use crate::scene::{MeshComponent, Scene};

pub(super) const RES: u32 = 16;
pub(super) const NORMALS: [Vec3; 6] = [
    Vec3::X,
    Vec3::NEG_X,
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::Z,
    Vec3::NEG_Z,
];

/// A closed box of six static walls around the origin, each `albedo` and glowing
/// `emissive`, the probe outside every wall's back face (#1615).
pub(super) fn closed_room(albedo: [f32; 3], emissive: [f32; 3]) -> Scene {
    let mut scene = Scene::new();
    scene.materials.insert(
        "wall".into(),
        MaterialAsset {
            base_color: albedo,
            emissive,
            ..MaterialAsset::default()
        },
    );
    for (i, axis) in NORMALS.iter().enumerate() {
        let scale = Vec3::splat(20.0) - axis.abs() * 19.5;
        add_box(&mut scene, &format!("Wall{i}"), *axis * 6.0, scale, "wall");
    }
    scene
}

pub(super) fn add_box(scene: &mut Scene, name: &str, pos: Vec3, scale: Vec3, material: &str) {
    let (vertices, indices) = crate::components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene.world.set_static(id, true);
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale, t.rotation) = (pos, scale, Quat::IDENTITY);
    }
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(material)));
}

pub(super) fn finite(sh: &Sh9) -> bool {
    sh.coeffs.iter().flatten().all(|c| c.is_finite())
}

/// With nothing clamping them, the bounces of a closed, bright, light-walled room
/// still converge: each pass adds less light than the one before, so the field
/// settles instead of growing per bounce.
#[test]
fn gpu_bounces_in_a_bright_closed_room_shrink_each_pass() {
    use crate::render::ibl::probe::bounce::field_delta;
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = closed_room([0.8; 3], [0.0; 3]);
    scene.materials.insert(
        "lamp".into(),
        MaterialAsset {
            emissive: [50.0; 3],
            ..MaterialAsset::default()
        },
    );
    add_box(
        &mut scene,
        "Lamp",
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(2.0, 0.5, 2.0),
        "lamp",
    );
    scene.probes.add_probe(Vec3::new(1.0, 0.0, 0.0));
    let (mut field, mut deltas) = (vec![Sh9::zero()], Vec::new());
    for pass in 1..=crate::render::MAX_BOUNCES {
        let capture = renderer.capture_light_probe(&scene, Vec3::new(1.0, 0.0, 0.0), RES, pass > 1);
        let next = vec![project_cubemap(&capture.cube)];
        assert!(finite(&next[0]), "pass {pass}");
        deltas.push(field_delta(&field, &next));
        scene.probes.probes_mut()[0].sh = next[0];
        field = next;
    }
    assert!(
        field[0].eval(Vec3::Y).x > std::f32::consts::PI,
        "the room is lit past 1"
    );
    for pair in deltas[1..].windows(2) {
        assert!(
            pair[1] < pair[0],
            "a bounce added more than the last: {deltas:?}"
        );
    }
}
