//! #666 — the default scene's NavMeshAgent bot, chasing a Player who flies past the
//! edge of `Floor_Plane`, must stay on the floor, stand on it (never half sunk), stop
//! at the edge nearest the Player instead of pushing into it, and follow the Player
//! back. `steering` runs the navigation tick alone; `play` runs the whole frame
//! (scripts, physics, nav) through `GameWorld`.

mod play;
mod steering;

use glam::Vec3;
use rusty::scene::Scene;

/// The floor collider's half extent: a `[15, 0.1, 15]` box scaled 2.5.
pub const FLOOR_HALF: f32 = 18.75;
/// The floor collider's top face.
pub const FLOOR_TOP: f32 = 0.05;
/// Enemy_1's base offset: its body is 2 m tall and centred on its origin.
pub const BASE_OFFSET: f32 = 1.0;

/// The tracked default scene, and Enemy_1's id in it.
pub fn default_scene() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene
        .load_from_file(rusty::scene::DEFAULT_SCENE_SOURCE)
        .expect("tracked default scene loads");
    let enemy = scene.find_entity_by_name("Enemy_1").expect("Enemy_1");
    (scene, enemy)
}

/// Enemy_1 stands on the floor: feet on its top face, inside its square.
pub fn assert_on_floor(pos: Vec3, when: &str) {
    let feet = pos.y - BASE_OFFSET;
    assert!(
        (feet - FLOOR_TOP).abs() < 0.02,
        "{when}: feet at y = {feet}, the floor's top is {FLOOR_TOP} (sunk or floating)"
    );
    assert!(
        pos.x.abs() <= FLOOR_HALF && pos.z.abs() <= FLOOR_HALF,
        "{when}: walked off the floor to {pos}"
    );
}

/// The XZ distance between two points.
pub fn planar(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}
