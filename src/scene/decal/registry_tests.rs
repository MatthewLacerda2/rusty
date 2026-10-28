//! The decal registry (#639): handles, owners and the fixed tick. The budgets:
//! `budget_tests.rs`.

use glam::{Quat, Vec3};

use crate::core::settings::decals::DecalLimits;
use crate::scene::decal::DecalSpec;
use crate::scene::Scene;

fn spawn_at(scene: &mut Scene, x: f32) -> u32 {
    scene
        .spawn_decal(Vec3::new(x, 0.0, 0.0), Vec3::Y, DecalSpec::default())
        .unwrap()
}

#[test]
fn ids_are_unique_and_survive_a_clear() {
    let mut s = Scene::new();
    let a = spawn_at(&mut s, 0.0);
    s.clear_decals();
    let b = spawn_at(&mut s, 0.0);
    assert!(a != 0 && b != a, "{a} {b}");
    assert!(!s.decals.remove(a, 0.0), "a cleared id names nothing");
    assert!(s.decals.remove(b, 0.0));
    assert!(s.decals.is_empty());
}

#[test]
fn an_owned_box_is_kept_in_its_owner_space_and_dropped_with_it() {
    let mut s = Scene::new();
    let door = s.add_entity("Door".into());
    s.world.transform_mut(door).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    let spec = DecalSpec {
        owner: Some(door),
        ..DecalSpec::default()
    };
    s.spawn_decal(Vec3::new(5.0, 1.0, 0.5), Vec3::Z, spec)
        .unwrap();
    {
        let mut t = s.world.transform_mut(door).unwrap();
        t.position = Vec3::new(0.0, 0.0, 3.0);
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    }
    let pose = s.decal_pose(&s.decals[0]).unwrap();
    // (0, 1, 0.5) in door space, turned 90° about Y and moved to (0, 0, 3).
    assert!(
        pose.centre().abs_diff_eq(Vec3::new(0.5, 1.0, 3.0), 1e-5),
        "{}",
        pose.centre()
    );
    s.world.set_active(door, false);
    assert!(
        s.decal_pose(&s.decals[0]).is_none(),
        "hidden while inactive"
    );
    s.decals.tick(0.0, |_| true);
    assert_eq!(s.decals.len(), 1, "kept while the owner lives");
    s.decals.tick(0.0, |_| false);
    assert!(s.decals.is_empty(), "dropped with its owner");
}

#[test]
fn a_dead_owner_stamps_nothing() {
    let mut s = Scene::new();
    let spec = DecalSpec {
        owner: Some(99),
        ..DecalSpec::default()
    };
    assert!(s.spawn_decal(Vec3::ZERO, Vec3::Y, spec).is_none());
    assert!(s.decals.is_empty());
}

#[test]
fn the_same_spawns_and_steps_give_the_same_decals() {
    let run = || {
        let mut s = Scene::new();
        s.decals.set_limits(DecalLimits {
            max_world: 24,
            ..DecalLimits::default()
        });
        for i in 0..64 {
            let spec = DecalSpec {
                lifetime: Some(0.5 + (i % 7) as f32 * 0.25),
                fade: 0.2,
                ..DecalSpec::default()
            };
            s.spawn_decal(Vec3::splat(i as f32), Vec3::Y, spec);
            s.decals.tick(1.0 / 60.0, |_| true);
        }
        s.decals.clone()
    };
    assert_eq!(run(), run());
}
