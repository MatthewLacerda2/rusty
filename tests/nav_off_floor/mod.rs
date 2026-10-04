//! #666, #747 — the default scene's NavMeshAgent bot on the fy_pool_day yard. Chasing
//! a Player who flies past the perimeter wall, it must stay in the yard, stand on the
//! ground (never half sunk), stop at the wall nearest the Player instead of pushing
//! into it, and follow the Player back; chasing one in the pool, it must go down a
//! ramp and back up one. `steering` runs the navigation tick alone; `play` runs the
//! whole frame (scripts, physics, nav) through `GameWorld`.

mod play;
mod pool;
mod steering;

use glam::Vec3;
use rusty::scene::default_scene::layout::{self, ground_at, RAMP_HALF_WIDTH, RAMP_RUN};
use rusty::scene::{apply_scene_data, to_scene_data, Scene};

/// The walls' inner faces: how far from the centre the yard is walkable (x, z).
pub const INNER: (f32, f32) = (
    layout::YARD_HALF.0 - layout::WALL_THICK,
    layout::YARD_HALF.1 - layout::WALL_THICK,
);
/// The bake erodes the grid cell beside a wall, so the walkable deck ends one cell
/// (1 m) short of the wall's face.
pub const WALL_MARGIN: f32 = 1.0;
/// Enemy_1's base offset: its body is 2 m tall and centred on its origin.
pub const BASE_OFFSET: f32 = 1.0;
/// A span's top is its cell's highest point, so on a ramp the feet ride up to one
/// cell's rise above the surface.
const RAMP_RIDE: f32 = layout::POOL_DEPTH / RAMP_RUN;

/// The default scene as the editor boots it (built, then through its saved
/// document), and Enemy_1's id in it.
pub fn default_scene() -> (Scene, u32) {
    let mut built = Scene::new();
    rusty::scene::default_scene::build(&mut built, rusty::scene::default_scene::BOT_SCRIPT);
    let mut scene = Scene::new();
    apply_scene_data(&mut scene, to_scene_data(&built));
    let enemy = scene.find_entity_by_name("Enemy_1").expect("Enemy_1");
    (scene, enemy)
}

/// Enemy_1 stands on the ground the layout puts under it, inside the walls.
pub fn assert_grounded(pos: Vec3, when: &str) {
    assert!(
        pos.x.abs() <= INNER.0 && pos.z.abs() <= INNER.1,
        "{when}: walked out of the yard to {pos}"
    );
    let feet = pos.y - BASE_OFFSET;
    let ground = ground_at(pos.x, pos.z).unwrap();
    assert!(
        feet > ground - 0.02 && feet < ground + RAMP_RIDE + 0.02,
        "{when}: feet at y = {feet}, the ground is at {ground} (sunk or floating)"
    );
}

/// Below the deck, Enemy_1 is in the pool, and where the pool slopes it is on a ramp.
pub fn assert_in_pool_only_by_ramp(pos: Vec3, when: &str) {
    let (px, pz) = layout::POOL_HALF;
    if pos.y - BASE_OFFSET > layout::DECK_TOP - 0.05 {
        return;
    }
    assert!(
        pos.x.abs() < px && pos.z.abs() < pz,
        "{when}: sunk at {pos}"
    );
    if pos.z.abs() > pz - RAMP_RUN + 0.5 {
        assert!(pos.x.abs() < RAMP_HALF_WIDTH, "{when}: dropped in at {pos}");
    }
}

/// Which ramp `pos` is on (−1 south, +1 north), if it is below the deck on one.
pub fn ramp_under(pos: Vec3) -> Option<i8> {
    let (_, pz) = layout::POOL_HALF;
    let below = pos.y - BASE_OFFSET < layout::DECK_TOP - 0.2;
    let sloped = pos.z.abs() > pz - RAMP_RUN + 0.5 && pos.z.abs() < pz;
    (below && sloped).then_some(if pos.z < 0.0 { -1 } else { 1 })
}

/// The XZ distance between two points.
pub fn planar(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

/// Enemy_1's authored stopping distance (1.5 m since #743): how far short of its
/// path's end it rests, so every "arrived near X" bound adds it.
pub fn reach(scene: &rusty::scene::Scene, enemy: u32) -> f32 {
    scene.world.nav_agent(enemy).unwrap().stopping_distance
}

/// The Player's spawn, where every chase comes home to.
pub fn home() -> Vec3 {
    Vec3::from(layout::PLAYER_SPAWN)
}

/// A point hovering 1.5 m over the middle of the pool floor.
pub fn pool_middle() -> Vec3 {
    Vec3::new(0.0, layout::POOL_FLOOR + 1.5, 0.0)
}
