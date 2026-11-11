//! src/physics/contact/fixture.rs — a body set rolling or standing on a floor,
//! for the contact tests (#2071).

use glam::{Quat, Vec3};

use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::physics::PhysicsWorld;
use crate::scene::authoring::{
    create_entity, default_collider, defaults::default_rigidbody, Primitive,
};
use crate::scene::Scene;

pub(super) const DT: f32 = 1.0 / 60.0;

/// The two floors a level is built from: a `Box` collider (a cuboid) and the
/// `Plane` primitive (a triangle-mesh collider), both topped at y = 0.
#[derive(Clone, Copy, Debug)]
pub(super) enum Floor {
    Box,
    Plane,
}

/// A static floor of `kind`, 600 m across.
pub(super) fn floor(scene: &mut Scene, kind: Floor) {
    let id = match kind {
        Floor::Box => {
            let id = scene.add_entity("Floor".to_string());
            let size = Vec3::new(600.0, 1.0, 600.0);
            let collider = ColliderComponent {
                shape: ColliderShape::Box { size },
                ..default_collider()
            };
            scene.world.set_collider(id, Some(collider));
            scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
            id
        }
        Floor::Plane => {
            let id = create_entity(scene, "Floor", Some(Primitive::Plane));
            // The primitive is 15 m square.
            scene.world.transform_mut(id).unwrap().scale = Vec3::new(40.0, 1.0, 40.0);
            id
        }
    };
    scene.world.set_static(id, true);
}

/// A dynamic body of `shape`, turned by `rot` and centred at `pos`, moving at
/// `vel` and spinning at `spin`.
pub(super) fn body(
    scene: &mut Scene,
    shape: ColliderShape,
    rot: Quat,
    pos: Vec3,
    vel: Vec3,
    spin: Vec3,
) -> u32 {
    let id = scene.add_entity("Body".to_string());
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.rotation) = (pos, rot);
    }
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let rb = RigidBodyComponent {
        velocity: vel,
        angular_velocity: spin,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
    id
}

/// `id`'s speed along the floor.
pub(super) fn floor_speed(scene: &Scene, id: u32) -> f32 {
    let v = scene.world.rigidbody(id).unwrap().velocity;
    Vec3::new(v.x, 0.0, v.z).length()
}

/// Step `scene` for `seconds` of fixed ticks, calling `each` after every tick.
pub(super) fn run(scene: &mut Scene, seconds: f32, mut each: impl FnMut(&Scene)) {
    let mut world = PhysicsWorld::from_scene(scene);
    for _ in 0..(seconds / DT).round() as usize {
        world.step(scene, DT);
        each(scene);
    }
}
