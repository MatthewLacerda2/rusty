mod play;
mod pool;
mod steering;

use glam::Vec3;
use rusty::edit::default_scene::layout::{self, ground_at, RAMP_HALF_WIDTH, RAMP_RUN};
use rusty::edit::{apply_scene_data, to_scene_data};
use rusty::scene::Scene;

pub const INNER: (f32, f32) = (
    layout::YARD_HALF.0 - layout::WALL_THICK,
    layout::YARD_HALF.1 - layout::WALL_THICK,
);
/// The bake erodes the grid cell beside a wall, so the walkable deck ends one cell
/// (1 m) short of the wall's face.
pub const WALL_MARGIN: f32 = 1.0;
/// Enemy_1's base offset: its body is 2 m tall and centred on its origin.
pub const BASE_OFFSET: f32 = 1.0;
/// How far the feet may sit off the ground: on a ramp too, they follow its surface,
/// not their cell's highest point.
const GROUND_TOLERANCE: f32 = 0.02;

pub fn default_scene() -> (Scene, u32) {
    default_scene_with(rusty::edit::default_scene::BOT_SCRIPT)
}

pub fn default_scene_with(bot_script: &str) -> (Scene, u32) {
    let mut built = Scene::new();
    rusty::edit::default_scene::build(&mut built, bot_script);
    let mut scene = Scene::new();
    apply_scene_data(&mut scene, to_scene_data(&built));
    let enemy = scene.find_entity_by_name("Enemy_1").expect("Enemy_1");
    (scene, enemy)
}

pub fn assert_grounded(pos: Vec3, when: &str) {
    assert!(
        pos.x.abs() <= INNER.0 && pos.z.abs() <= INNER.1,
        "{when}: walked out of the yard to {pos}"
    );
    let feet = pos.y - BASE_OFFSET;
    let ground = ground_at(pos.x, pos.z).unwrap();
    assert!(
        (feet - ground).abs() < GROUND_TOLERANCE,
        "{when}: feet at y = {feet}, the ground is at {ground} (sunk or floating)"
    );
}

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

pub fn ramp_under(pos: Vec3) -> Option<i8> {
    let (_, pz) = layout::POOL_HALF;
    let below = pos.y - BASE_OFFSET < layout::DECK_TOP - 0.2;
    let sloped = pos.z.abs() > pz - RAMP_RUN + 0.5 && pos.z.abs() < pz;
    (below && sloped).then_some(if pos.z < 0.0 { -1 } else { 1 })
}

pub fn planar(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

/// Enemy_1's authored stopping distance (1.5 m): how far short of its
/// path's end it rests, so every "arrived near X" bound adds it.
pub fn reach(scene: &rusty::scene::Scene, enemy: u32) -> f32 {
    scene.world.nav_agent(enemy).unwrap().stopping_distance
}

pub fn home() -> Vec3 {
    Vec3::from(layout::PLAYER_SPAWN)
}

pub fn pool_middle() -> Vec3 {
    Vec3::new(0.0, layout::POOL_FLOOR + 1.5, 0.0)
}
