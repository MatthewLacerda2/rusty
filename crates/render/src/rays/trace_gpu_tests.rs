use glam::{Quat, Vec3};

use super::fixtures::{rays, trs, Fixture};
use super::probe::trace;
use super::walk::trace_closest;
use rusty_components::RenderFace;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

/// A room, a stair, a few turned cubes and a floor: every object static, the cubes
/// drawn from both sides.
pub(super) fn level() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let both = rusty_components::MaterialAsset {
        render_face: RenderFace::Both,
        ..Default::default()
    };
    scene.materials.insert("both".into(), both);
    let mut place = |kind, world: glam::Mat4, material: Option<&str>| {
        let id = create_entity(&mut scene, "Solid", Some(kind));
        let (scale, rotation, position) = world.to_scale_rotation_translation();
        if let Some(mut t) = scene.world.transform_mut(id) {
            (t.position, t.rotation, t.scale) = (position, rotation, scale);
        }
        let material = material.map(|m| rusty_components::MaterialComponent::new(m.to_string()));
        scene.world.set_material(id, material);
        scene.world.set_static(id, true);
        id
    };
    place(
        Primitive::Plane,
        trs(Vec3::splat(2.0), Quat::IDENTITY, Vec3::ZERO),
        None,
    );
    for i in 0..6 {
        let at = Vec3::new(3.0, 0.25 * i as f32, 0.3 * i as f32);
        place(
            Primitive::Box,
            trs(Vec3::new(2.0, 0.25, 0.3), Quat::IDENTITY, at),
            None,
        );
    }
    for i in 0..9 {
        let turn = Quat::from_rotation_y(0.4 * i as f32) * Quat::from_rotation_x(0.2);
        let at = Vec3::new((i % 3) as f32 * 1.5 - 3.0, 1.0, (i / 3) as f32 * 1.5);
        place(
            Primitive::Box,
            trs(Vec3::splat(0.7), turn, at),
            Some("both"),
        );
    }
    scene
}

#[test]
fn gpu_the_shader_hits_what_the_cpu_walk_hits() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let scene = level();
    r.extract_scene(&scene);
    let packed = r.ray_tree(&scene).expect("extracted").packed();
    assert!(packed.instances.len() >= 15, "every static solid is traced");
    let mut eyes = Fixture::one(0, glam::Mat4::IDENTITY);
    eyes.eyes = (Vec3::new(-4.0, 0.2, -1.0), Vec3::new(5.0, 3.0, 4.0));
    let rays = rays(&eyes, 2000);
    let gpu = trace(&mut r, &scene, &rays);
    let (mut hits, mut ties) = (0, 0);
    for (&(o, d, reach), &(theirs, any)) in rays.iter().zip(&gpu) {
        let ours = trace_closest(&packed, o, d, reach);
        assert_eq!(any, theirs.is_some(), "trace_any and trace_closest agree");
        hits += usize::from(ours.is_some());
        match (ours, theirs) {
            (Some(a), Some(b)) if (a.slot, a.tri) == (b.slot, b.tri) => {
                assert!((a.t - b.t).abs() <= 1e-4 * a.t.max(1.0), "{a:?} vs {b:?}");
                assert!((a.u - b.u).abs() < 1e-3 && (a.v - b.v).abs() < 1e-3);
            }
            (None, None) => {}
            (a, b) => {
                let edge = |h: super::walk::Hit| h.u < 1e-3 || h.v < 1e-3 || h.u + h.v > 1.0 - 1e-3;
                assert!(
                    a.into_iter().chain(b).all(edge),
                    "{a:?} vs {b:?} off an edge"
                );
                ties += 1;
            }
        }
    }
    assert!(hits > 1000, "most rays hit: {hits}");
    assert!(ties <= 10, "{ties} edge ties of 2000 rays");
}
