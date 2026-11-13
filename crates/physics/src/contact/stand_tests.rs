//! crates/physics/src/contact/stand_tests.rs — a cylinder stood on its cap stays put,
//! and a rolling one comes to rest (#2071).
//!
//! The side of a cylinder rolls as a hull (`contact::side`); its cap is still the
//! true cylinder, so an upright one stands flat on any floor.

use glam::{Quat, Vec3};

use super::fixture::{body, floor, floor_speed, run, Floor};
use rusty_components::ColliderShape;
use rusty_scene::Scene;

#[test]
fn an_upright_cylinder_stands_flat_on_its_cap() {
    for ground in [Floor::Box, Floor::Plane] {
        for (radius, height) in [(0.4, 1.0), (1.0, 0.2), (0.1, 2.0)] {
            let mut scene = Scene::new();
            floor(&mut scene, ground);
            let shape = ColliderShape::Cylinder { radius, height };
            let pos = Vec3::Y * (height / 2.0);
            let id = body(
                &mut scene,
                shape,
                Quat::IDENTITY,
                pos,
                Vec3::ZERO,
                Vec3::ZERO,
            );
            run(&mut scene, 10.0, |_| {});
            let t = scene.world.transform(id).unwrap();
            let up = (t.rotation * Vec3::Y).y;
            let label = format!("r {radius} h {height} on {ground:?}");
            assert!(up > 0.9999, "{label} tipped: up {up}");
            // A tall thin one settles with a wobble of a centimetre a second, as before #2071.
            let drift = Vec3::new(t.position.x, 0.0, t.position.z).length();
            assert!(drift < 0.01, "{label} wandered {drift} m");
            assert!(floor_speed(&scene, id) < 1e-3, "{label} never settled");
            assert!(
                (t.position.y - height / 2.0).abs() < 0.01,
                "{label} sank: y {}",
                t.position.y
            );
        }
    }
}

#[test]
fn a_rolling_cylinder_comes_to_rest() {
    for ground in [Floor::Box, Floor::Plane] {
        let mut scene = Scene::new();
        floor(&mut scene, ground);
        let shape = ColliderShape::Cylinder {
            radius: 0.4,
            height: 1.0,
        };
        let lie = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let spin = Vec3::Z * (-2.0 / 0.4);
        let id = body(&mut scene, shape, lie, Vec3::Y * 0.4, Vec3::X * 2.0, spin);
        run(&mut scene, 60.0, |_| {});
        let speed = floor_speed(&scene, id);
        assert!(
            speed < 0.01,
            "on {ground:?} still rolling at {speed} m/s after a minute"
        );
    }
}

#[test]
fn the_same_roll_lands_on_the_same_bits() {
    let roll = || {
        let mut scene = Scene::new();
        floor(&mut scene, Floor::Box);
        let shape = ColliderShape::Cylinder {
            radius: 0.4,
            height: 1.0,
        };
        let lie = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let id = body(
            &mut scene,
            shape,
            lie,
            Vec3::Y * 0.4,
            Vec3::X,
            Vec3::Z * -2.5,
        );
        run(&mut scene, 5.0, |_| {});
        let t = scene.world.transform(id).unwrap();
        (
            t.position.to_array().map(f32::to_bits),
            t.rotation.to_array().map(f32::to_bits),
        )
    };
    assert_eq!(roll(), roll());
}
