//! A driven joint is touched only when its target or drive changes (#1895):
//! touching one wakes its bodies, so a driven body holding still can sleep.

use glam::Vec3;

use crate::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_scene::skeleton::rig::armature_owner;
use rusty_scene::skeleton::{Drive, RagdollOptions, HITBOX_NAME};
use rusty_scene::Scene;

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
            ..rusty_scene::authoring::default_collider()
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
    assert_eq!(
        scene.set_drive(rig, spine, [Some(drive.strength), Some(drive.damping)]),
        2
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..3 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    (scene, physics, spine, rig)
}

#[test]
fn an_unchanged_drive_touches_no_joint_so_a_still_body_can_sleep() {
    let (mut scene, mut physics, ..) = driven_spine();
    assert_eq!(physics.driven.len(), 2, "the spine and head joints");
    assert_eq!(
        physics.drive_joints(&mut scene),
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
    assert_eq!(
        physics.drive_joints(&mut scene),
        1,
        "the spine's new target"
    );
    let drive = Drive {
        strength: 900.0,
        damping: 60.0,
    };
    scene.set_drive(rig, spine, [Some(drive.strength), Some(drive.damping)]);
    assert_eq!(
        physics.drive_joints(&mut scene),
        2,
        "both joints' new drive"
    );
}

#[test]
fn a_drive_set_on_the_joint_after_enable_chases_the_animation() {
    // `Joint.SetDriveStrength` (or the inspector) on a fully ragdolled bone,
    // not `Ragdoll.SetDrive`: the drive still tracks its animated pose.
    let (mut scene, mut physics, spine, rig) = driven_spine();
    let head = scene.find_bone(rig, "head").unwrap();
    scene.world.joint_mut(head).unwrap().drive_strength = 0.0;
    scene.set_ragdoll(rig, true, |_| None).unwrap();
    physics.step(&mut scene, 1.0 / 60.0);
    assert!(
        !scene.ragdolls.is_tracked(head),
        "Enable alone tracks nothing undriven"
    );
    scene.world.joint_mut(head).unwrap().drive_strength = 300.0;
    physics.step(&mut scene, 1.0 / 60.0);
    assert!(
        scene.ragdolls.is_tracked(head),
        "tracked where the drive is read"
    );
    assert!(scene.ragdolls.is_tracked(spine));
}

#[test]
fn a_rebuilt_joint_gets_its_motors_back() {
    // A powered door (a hinge to the world, driven to rest): `Joint.SetLimits`
    // rebuilds it, rapier hands it back motorless, and its target is unchanged.
    let mut scene = Scene::new();
    let door = scene.add_entity("Door".to_string());
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..rusty_scene::authoring::default_collider()
    };
    scene.world.set_collider(door, Some(collider));
    let rb = rusty_scene::authoring::defaults::default_rigidbody();
    scene.world.set_rigidbody(door, Some(rb));
    let joint = rusty_components::JointComponent {
        kind: rusty_components::JointKind::Hinge,
        use_limits: true,
        drive_strength: 300.0,
        drive_damping: 35.0,
        ..Default::default()
    };
    scene.world.set_joint(door, Some(joint));
    let mut physics = PhysicsWorld::from_scene(&scene);
    let motors = |p: &PhysicsWorld| {
        let live = p.impulse_joints.get(p.joints[&door].handle).unwrap();
        live.data.motor_axes
    };
    physics.step(&mut scene, 1.0 / 60.0);
    assert!(!motors(&physics).is_empty(), "driven");
    scene.world.joint_mut(door).unwrap().limits.y += 0.1;
    physics.step(&mut scene, 1.0 / 60.0);
    assert!(
        !motors(&physics).is_empty(),
        "the rebuilt joint is driven too"
    );
}
