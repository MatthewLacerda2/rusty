//! A primitive arena the environment-query tests share (#1892): a 40 × 40 m floor
//! baked into a navmesh, a physics world over it, and a camera.

use glam::{Vec2, Vec3};

use super::{Anchor, QueryWorld};
use crate::navigation::test_support::{add_box, add_floor};
use crate::navigation::{NavBounds, NavigationGraph};
use crate::physics::PhysicsWorld;
use crate::scene::{Camera, NavMeshAgentComponent, Scene};

/// The arena and what a query reads from it.
pub struct Arena {
    pub scene: Scene,
    pub nav: NavigationGraph,
    pub physics: PhysicsWorld,
    pub camera: Camera,
}

/// The arena's half extent, metres: the floor spans `±HALF` in X and Z.
pub const HALF: f32 = 20.0;

impl Arena {
    /// An open floor; `build` adds walls and the rest before the bake.
    pub fn new(build: impl FnOnce(&mut Scene)) -> Self {
        let mut scene = Scene::new();
        add_floor(&mut scene, -HALF, HALF, -HALF, HALF);
        build(&mut scene);
        scene.nav_settings.bounds = Some(NavBounds::new(-HALF, HALF, -HALF, HALF));
        let mut nav = NavigationGraph::new(-HALF, HALF, -HALF, HALF, 1.0);
        nav.bake(&scene);
        let physics = PhysicsWorld::from_scene(&scene);
        // High above the origin, looking down +X.
        let camera = Camera::new(Vec3::new(-10.0, 1.5, 0.0), 0.0, 0.0);
        Self {
            scene,
            nav,
            physics,
            camera,
        }
    }

    pub fn world(&self) -> QueryWorld<'_> {
        QueryWorld {
            scene: &self.scene,
            nav: &self.nav,
            physics: Some(&self.physics),
            view: &self.camera,
            screen: Vec2::new(1600.0, 900.0),
        }
    }

    /// An active agent standing at `at`, its target where it stands.
    pub fn agent(&mut self, at: Vec3) -> u32 {
        let id = self.scene.add_entity("agent".to_string());
        self.scene.world.transform_mut(id).expect("new").position = at;
        let agent = NavMeshAgentComponent {
            active: true,
            target: at,
            speed: 3.5,
            acceleration: 8.0,
            ..Default::default()
        };
        self.scene.world.set_nav_agent(id, Some(agent));
        id
    }

    /// `id` as a query context.
    pub fn anchor(&self, id: u32) -> Anchor {
        Anchor::of_entity(&self.scene, id)
    }
}

/// A 3 m high wall over the world AABB `min.xz .. max.xz`.
pub fn wall(scene: &mut Scene, min: (f32, f32), max: (f32, f32)) -> u32 {
    add_box(
        scene,
        Vec3::new(min.0, 0.0, min.1),
        Vec3::new(max.0, 3.0, max.1),
    )
}
