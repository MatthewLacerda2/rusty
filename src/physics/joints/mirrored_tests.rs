//! src/physics/joints/mirrored_tests.rs — a joint under a mirror (#1125): a hinge
//! with asymmetric limits under a parent scaled −1 on X moves as the mirror image
//! of the unmirrored one, whatever its axis, stopping on the mirror-image side;
//! and a joint whose own mirror cancels its parent's moves as the plain joint
//! with the same world matrix does.

use glam::{Quat, Vec2, Vec3};

use super::super::compound::world_pose;
use super::super::world::PhysicsWorld;
use crate::components::{
    ColliderComponent, ColliderShape, JointComponent, JointKind, RigidBodyComponent,
};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;
/// The X mirror, as a scale.
const MIRROR: Vec3 = Vec3::new(-1.0, 1.0, 1.0);
/// Where the cube sits off its pivot: off every axis tested, so gravity turns it
/// about each.
const OFFSET: Vec3 = Vec3::new(1.0, 0.0, 0.5);

/// A dynamic unit cube at local `pos` with its own `scale`, under a static
/// parent scaled `parent_scale`, hinged to the world at its pivot (`-OFFSET`
/// from its centre) about local `axis`, limited to `[-15°, 50°]`.
fn hinge(parent_scale: Vec3, pos: Vec3, scale: Vec3, axis: Vec3) -> (Scene, u32) {
    let mut scene = Scene::new();
    let parent = scene.add_entity("Parent".to_string());
    scene.world.transform_mut(parent).unwrap().scale = parent_scale;
    let id = scene.add_entity("Cube".to_string());
    scene.set_parent(id, Some(parent)).unwrap();
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (pos, scale);
    }
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        is_kinematic: false,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    let joint = JointComponent {
        kind: JointKind::Hinge,
        anchor: -OFFSET,
        axis,
        use_limits: true,
        limits: Vec2::new(-15.0, 50.0),
        ..Default::default()
    };
    scene.world.set_joint(id, Some(joint));
    (scene, id)
}

/// The cube's world position and rotation after a second and a half.
fn swung((mut scene, id): (Scene, u32)) -> (Vec3, Quat) {
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..90 {
        world.step(&mut scene, DT);
    }
    let pose = world_pose(&scene, id).unwrap();
    (pose.pos, pose.rot)
}

fn same(a: Quat, b: Quat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-5
}

#[test]
fn a_mirrored_hinge_swings_as_the_mirror_image() {
    let axes = [Vec3::X, Vec3::Z, Vec3::new(1.0, 0.0, 1.0).normalize()];
    for axis in axes {
        let (pos, rot) = swung(hinge(Vec3::ONE, OFFSET, Vec3::ONE, axis));
        assert!(rot.angle_between(Quat::IDENTITY) > 0.2, "{axis}: it swung");
        let (mpos, mrot) = swung(hinge(MIRROR, OFFSET, Vec3::ONE, axis));
        // The mirror image: position reflected, the rotation seen through it.
        let image = Quat::from_xyzw(rot.x, -rot.y, -rot.z, rot.w);
        assert!(mpos.abs_diff_eq(pos * MIRROR, 1e-3), "{axis}: at {mpos}");
        assert!(same(mrot, image), "{axis}: {mrot:?}, image {image:?}");
    }
}

#[test]
fn a_double_mirror_swings_as_the_plain_joint() {
    let axis = Vec3::new(1.0, 0.0, 1.0).normalize();
    // Parent and own mirrors cancel: the same world matrix as a plain cube at
    // the reflected spot, anchor and axis unchanged.
    let (pos, rot) = swung(hinge(MIRROR, OFFSET, MIRROR, axis));
    let plain = swung(hinge(Vec3::ONE, OFFSET * MIRROR, Vec3::ONE, axis));
    assert!(
        pos.abs_diff_eq(plain.0, 1e-4),
        "at {pos}, plain {}",
        plain.0
    );
    assert!(same(rot, plain.1), "{rot:?}, plain {:?}", plain.1);
}
