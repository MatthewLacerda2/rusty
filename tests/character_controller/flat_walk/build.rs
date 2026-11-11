//! The walkers and floors `flat_walk` builds.

use glam::Vec3;
use rusty::components::{CharacterControllerComponent, ColliderShape};
use rusty::scene::authoring::rendering::mesh::set_subdivisions;
use rusty::scene::authoring::{add_component, create_entity, ComponentKind, Primitive};
use rusty::scene::Scene;

use super::{Floor, LENGTH, START_Z};
use crate::play::attach_script;

/// A default capsule, and a 1.8 m one of 0.4 m radius with its pivot at the
/// feet (its centre 0.9 m up).
pub(super) fn capsules() -> [CharacterControllerComponent; 2] {
    let tall = CharacterControllerComponent {
        height: 1.8,
        radius: 0.4,
        center: Vec3::new(0.0, 0.9, 0.0),
        ..Default::default()
    };
    [CharacterControllerComponent::default(), tall]
}

/// One lane: a controller whose script walks it +Z at `speed`.
pub(super) struct Lane {
    pub(super) id: u32,
    pub(super) speed: f32,
    pub(super) label: String,
}

pub(super) fn add_lane(
    scene: &mut Scene,
    x: f32,
    cc: CharacterControllerComponent,
    speed: f32,
    stick: f32,
) -> Lane {
    let id = scene.add_entity("Walker".to_string());
    // Feet on the floor plus the skin.
    let y = cc.skin_width - (cc.center.y - cc.height * 0.5);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, y, START_Z);
    let label = format!("{speed} m/s, stick {stick}, height {}", cc.height);
    scene.world.set_character_controller(id, Some(cc));
    let source = format!(
        "local W = {{}}\nfunction W.Update(id, dt)\n  \
         CharacterController.Move(id, 0, {stick}, {speed} * dt)\nend\nreturn W\n"
    );
    attach_script(scene, id, &format!("flat_walk_{id}.lua"), &source);
    Lane { id, speed, label }
}

/// `floor` as the editor builds it, its top face at y = 0.
pub(super) fn add_floor(scene: &mut Scene, (name, primitive, scale, y, is_static, cuts): Floor) {
    let floor = create_entity(scene, name, Some(primitive));
    let mut t = scene.world.transform_mut(floor).unwrap();
    t.position = Vec3::new(0.0, y, START_Z + LENGTH * 0.5 - 10.0);
    t.scale = scale;
    drop(t);
    assert!(add_component(scene, floor, ComponentKind::Collider));
    scene.world.set_static(floor, is_static);
    if primitive == Primitive::Plane {
        // A plane's collider is its mesh, as Unity ships one (the default
        // added is a unit box).
        let shape = ColliderShape::Mesh {
            convex: false,
            local_min: Vec3::new(-7.5, 0.0, -7.5),
            local_max: Vec3::new(7.5, 0.0, 7.5),
        };
        scene.world.collider_mut(floor).unwrap().shape = shape;
        let mut mesh = scene.world.mesh_mut(floor).unwrap();
        set_subdivisions(&mut mesh, cuts).unwrap();
    }
}
