//! A driven joint is touched only when its target or drive changes (#1895):
//! touching one wakes its bodies, so a driven body holding still can sleep.

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape};
use crate::physics::PhysicsWorld;
use crate::scene::skeleton::fixture::armature_owner;
use crate::scene::skeleton::{Drive, RagdollOptions, HITBOX_NAME};
use crate::scene::Scene;

/// A ragdolled rig with its spine branch simulated and driven: the scene, the
/// physics world after a few steps, the spine and the rig.
fn driven_spine() -> (Scene, PhysicsWorld, u32, u32) {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    for name in ["hips", "spine", "head"] {
        let bone = scene.find_bone(rig, name).unwrap();
        let hb = scene.add_entity(HITBOX_NAME.to_string());
        scene.set_parent(hb, Some(bone)).unwrap();
        let shape = ColliderShape::Box {
            size: Vec3::splat(0.2),
        };
        let c = ColliderComponent {
            shape,
            ..crate::scene::authoring::default_collider()
        };
        scene.world.set_collider(hb, Some(c));
    }
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    let spine = scene.find_bone(rig, "spine").unwrap();
    scene.set_simulated(rig, spine, true, &|_| None);
    let drive = Drive {
        strength: 400.0,
        damping: 40.0,
    };
    assert_eq!(scene.set_drive(rig, spine, drive), 2);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..3 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    (scene, physics, spine, rig)
}

#[test]
fn an_unchanged_drive_touches_no_joint_so_a_still_body_can_sleep() {
    let (scene, mut physics, ..) = driven_spine();
    assert_eq!(physics.driven.len(), 2, "the spine and head joints");
    assert_eq!(
        physics.drive_joints(&scene),
        0,
        "nothing changed: no wake-up"
    );
}

#[test]
fn a_changed_target_or_drive_is_set_again() {
    let (mut scene, mut physics, spine, rig) = driven_spine();
    let (pos, rot) = scene.ragdolls.animated(spine).unwrap();
    let bent = glam::Quat::from_rotation_x(0.4) * rot;
    scene.ragdolls.note_animated(spine, (pos, bent));
    assert_eq!(physics.drive_joints(&scene), 1, "the spine's new target");
    let drive = Drive {
        strength: 900.0,
        damping: 60.0,
    };
    scene.set_drive(rig, spine, drive);
    assert_eq!(physics.drive_joints(&scene), 2, "both joints' new drive");
}
