//! A primitive arena the perception tests share (#1900): a floor, a physics world
//! over it, perceivers facing +Z and targets on the [`PLAYER`] layer.

use glam::Vec3;

use super::{drain_updates, step, SightRays};
use rusty_components::{
    AIPerceptionComponent, ColliderComponent, ColliderShape, Memory, PerceptionUpdate,
};
use rusty_navigation::test_support::{add_box, add_floor};
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

/// One fixed tick, seconds.
pub const DT: f32 = 1.0 / 60.0;
/// The layer targets stand on.
pub const PLAYER: u8 = 3;

/// The arena, its physics and the sight budget.
pub struct Arena {
    pub scene: Scene,
    pub physics: PhysicsWorld,
    pub rays: SightRays,
}

impl Arena {
    /// A 100 m floor, in Play (the senses sense nothing outside it); `build`
    /// adds walls, perceivers and targets before physics.
    pub fn new(build: impl FnOnce(&mut Scene)) -> Self {
        let mut scene = Scene::new();
        scene.set_playing(true);
        add_floor(&mut scene, -50.0, 50.0, -50.0, 50.0);
        build(&mut scene);
        let physics = PhysicsWorld::from_scene(&scene);
        Self {
            scene,
            physics,
            rays: SightRays::default(),
        }
    }

    /// Run `ticks` perception steps.
    pub fn step(&mut self, ticks: usize) {
        for _ in 0..ticks {
            step(&mut self.scene, Some(&self.physics), &mut self.rays, DT);
        }
    }

    /// Take the pending updates.
    pub fn updates(&mut self) -> Vec<(u32, PerceptionUpdate)> {
        drain_updates(&mut self.scene)
    }

    /// `id`'s memory, cloned.
    pub fn memory(&self, id: u32) -> Memory {
        let p = self.scene.world.ai_perception(id).expect("a perceiver");
        p.memory.clone()
    }

    /// Whether `id` sees `target` now.
    pub fn sees(&self, id: u32, target: u32) -> bool {
        let p = self.scene.world.ai_perception(id).expect("a perceiver");
        p.memory.in_sight.contains(&target)
    }

    /// Move `id` to `at`.
    pub fn put(&mut self, id: u32, at: Vec3) {
        self.scene.world.transform_mut(id).expect("alive").position = at;
    }
}

/// A perceiver standing at `at`, facing +Z, looking for the [`PLAYER`] layer.
pub fn perceiver(scene: &mut Scene, at: Vec3) -> u32 {
    let id = scene.add_entity("bot".to_string());
    scene.world.transform_mut(id).expect("new").position = at;
    let p = AIPerceptionComponent {
        sight_mask: 1 << PLAYER,
        ..Default::default()
    };
    scene.world.set_ai_perception(id, Some(p));
    id
}

/// A 0.5 × 1.8 × 0.5 m moving body on the [`PLAYER`] layer, its feet at `at`.
pub fn target(scene: &mut Scene, at: Vec3) -> u32 {
    let id = scene.add_entity("player".to_string());
    scene.world.transform_mut(id).expect("new").position = at + Vec3::Y * 0.9;
    scene.world.set_layer(id, PLAYER);
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box {
        size: Vec3::new(0.5, 1.8, 0.5),
    };
    scene.world.set_collider(id, Some(col));
    id
}

/// A 3 m high static wall over `x0..x1` × `z0..z1`.
pub fn wall(scene: &mut Scene, (x0, x1): (f32, f32), (z0, z1): (f32, f32)) -> u32 {
    add_box(scene, Vec3::new(x0, 0.0, z0), Vec3::new(x1, 3.0, z1))
}
