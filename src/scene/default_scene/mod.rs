//! src/scene/default_scene/ — the default scene, built in Rust (#667).
//!
//! The ONE layout the editor seeds into `project/scenes/default.scene` on first
//! launch and the headless harness plays, so what an agent tests is what the user
//! sees. A scene this small (a few primitives, a light, two scripts) gains nothing
//! from being a data file, and as code it moves with the engine instead of rotting.
//!
//! Everything is built through the shared authoring ops (`create_entity`,
//! `add_component`, the per-component `set_*` ops, `authoring::material`) — the same
//! path the editor and the Lua API take — never a parallel one.
//!
//! The layout is a greybox of **fy_pool_day**, Counter-Strike's fight yard (#747),
//! built from boxes and planes only: a walled yard, a sunken pool between the two
//! spawns with a ramp at each end, translucent water with no collider, a raised
//! jacuzzi you jump into, and crates for cover. It exercises layered navigation
//! (the bot chases down one ramp and up the other), character-controller slopes,
//! and shadows at level scale. Enemy_1 starts behind a crate, so its first chase
//! paths **around** it.
//!
//! The straight line from the Player to Enemy_1 stays walkable and clear up to 4 m
//! from the enemy: the bot-player (`project/scripts/bot_player.lua`) walks it with
//! no pathing, down one ramp, across the bottom and up the other.
//!
//! Submodules: `layout` (every measurement, shared with the tests), `yard` (deck,
//! walls, crates), `pool` (pool, ramps, water, jacuzzi), `looks` (materials, the
//! checker recipe, the default shader recipe), `seed` (bakes those into
//! `project/assets/` — the only I/O here).

pub mod layout;
mod looks;
mod pool;
mod seed;
mod yard;

use glam::{Quat, Vec3};

use crate::scene::authoring::{
    self, add_component, animator, character_controller as cc, collider, create_entity, nav_agent,
    ComponentKind, Primitive,
};
use crate::scene::{ColliderShape, MaterialComponent, Scene, ScriptComponent};

pub use looks::{checker_recipe, shader_recipe, CHECKER_MAP, SHADER_NAME};
pub use seed::{seed_default_assets, seed_default_assets_into};

/// The bundled player brain (movement + camera + weapon), attached to the Player.
pub const PLAYER_CONTROLLER_SCRIPT: &str = "project/assets/scripts/player_controller.lua";
/// The bundled enemy brain the editor's default scene attaches to Enemy_1.
pub const BOT_SCRIPT: &str = "project/assets/scripts/bot.lua";
/// How far from the Player (centre to centre) Enemy_1 stops: its 0.65 m half-width
/// plus the Player's 0.5 m radius plus clearance, so the two never overlap (#743).
pub const ENEMY_STOPPING_DISTANCE: f32 = 1.5;

/// Build the default scene into `scene`. `bot_script` is Enemy_1's Lua brain; empty
/// leaves it unscripted (the harness's default, so a test drives the enemy itself).
/// Pure: no I/O, no RNG — the maps and shader it names are seeded separately.
///
/// Ids are stable (Pool_Floor 1, Player 2, ramps 3–4, Enemy_1 5, Sun 6); the deck,
/// walls, water, jacuzzi and crates follow from 7.
pub fn build(scene: &mut Scene, bot_script: &str) {
    looks::define_all(&mut scene.materials);
    pool::add_floor(scene);
    add_player(scene);
    pool::add_ramps(scene);
    add_enemy(scene, bot_script);
    add_sun(scene);
    yard::add_deck(scene);
    yard::add_walls(scene);
    pool::add_water(scene);
    pool::add_jacuzzi(scene);
    yard::add_crates(scene);
    scene.update_all_colliders();
}

/// A static box spanning `min`..`max` wearing `material`, with a unit box collider
/// the transform scales.
fn block(scene: &mut Scene, name: &str, min: Vec3, max: Vec3, material: &str) -> u32 {
    let at = ((min + max) * 0.5, max - min);
    let unit = ColliderShape::Box { size: Vec3::ONE };
    solid(scene, name, Primitive::Box, at, unit, material)
}

/// A static mesh primitive at `pos`/`scale` wearing `material`, with a collider of
/// `shape` (unscaled — the transform scales it, as for any authored collider).
fn solid(
    scene: &mut Scene,
    name: &str,
    primitive: Primitive,
    (pos, scale): (Vec3, Vec3),
    shape: ColliderShape,
    material: &str,
) -> u32 {
    let id = create_entity(scene, name, Some(primitive));
    place(scene, id, pos, scale);
    scene.world.set_static(id, true);
    add_component(scene, id, ComponentKind::Collider);
    collider::set_shape(&mut scene.world.collider_mut(id).unwrap(), shape);
    wear(scene, id, material);
    id
}

fn place(scene: &mut Scene, id: u32, pos: Vec3, scale: Vec3) {
    let mut t = scene.world.transform_mut(id).unwrap();
    t.position = pos;
    t.scale = scale;
}

/// Point entity `id` at library material `key`.
fn wear(scene: &mut Scene, id: u32, key: &str) {
    let material = key.to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent { material }));
}

fn attach_script(scene: &mut Scene, id: u32, path: &str) {
    scene.world.scripts_mut(id).unwrap().push(ScriptComponent {
        path: path.to_string(),
        is_loaded: false,
        ..Default::default()
    });
}

/// A character capsule of `height` × `radius` before scale (#451).
fn character(scene: &mut Scene, id: u32, height: f32, radius: f32) {
    add_component(scene, id, ComponentKind::CharacterController);
    let mut c = scene.world.character_controller_mut(id).unwrap();
    cc::set_height(&mut c, height);
    cc::set_radius(&mut c, radius);
}

/// Player (id 2): a 1.6 m × 0.5 m cylinder — the unit primitive scaled, its capsule
/// scaling with it.
fn add_player(scene: &mut Scene) {
    let id = create_entity(scene, "Player", Some(Primitive::Cylinder));
    place(
        scene,
        id,
        Vec3::from(layout::PLAYER_SPAWN),
        Vec3::new(1.0, 1.6, 1.0),
    );
    character(scene, id, 1.0, 0.5);
    wear(scene, id, looks::PLAYER);
    attach_script(scene, id, PLAYER_CONTROLLER_SCRIPT);
}

/// Enemy_1 (id 5): a 2 m × 1.3 m box chasing the Player on the navmesh, from the +z
/// spawn behind its cover crate; unscripted, it heads for the pool's middle (its
/// authored target, the origin), down the north ramp. Its body is
/// centred on its origin, so the agent stands it 1 m above its feet (#666). The agent
/// is authored here, active, and `bot.lua` never overrides it (#743): its stopping
/// distance is measured centre to centre, so it clears both bodies — the box's
/// corner (half-diagonal ≈ 0.92 m) plus the Player's 0.5 m radius, with room to spare.
fn add_enemy(scene: &mut Scene, bot_script: &str) {
    let id = create_entity(scene, "Enemy_1", Some(Primitive::Box));
    place(
        scene,
        id,
        Vec3::from(layout::ENEMY_SPAWN),
        Vec3::new(1.3, 2.0, 1.3),
    );
    character(scene, id, 1.0, 0.5);
    add_component(scene, id, ComponentKind::Animator);
    {
        let mut a = scene.world.animator_mut(id).unwrap();
        animator::set_clip(&mut a, "Walk".to_string());
        animator::set_speed(&mut a, 3.0);
    }
    add_component(scene, id, ComponentKind::NavMeshAgent);
    {
        let mut a = scene.world.nav_agent_mut(id).unwrap();
        nav_agent::set_active(&mut a, true);
        nav_agent::set_radius(&mut a, 0.65);
        nav_agent::set_speed(&mut a, 3.5);
        nav_agent::set_acceleration(&mut a, 8.0);
        nav_agent::set_stopping_distance(&mut a, ENEMY_STOPPING_DISTANCE);
        nav_agent::set_base_offset(&mut a, 1.0);
        nav_agent::set_target(&mut a, Vec3::ZERO);
    }
    wear(scene, id, looks::ENEMY);
    if !bot_script.is_empty() {
        attach_script(scene, id, bot_script);
    }
}

/// Sun (id 6): a warm directional light from high to one side, so the walls and
/// crates drop shadows across the deck and into the pool.
fn add_sun(scene: &mut Scene) {
    let id = create_entity(scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.set_static(id, true);
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = Vec3::new(0.0, 12.0, 0.0);
        t.rotation = Quat::from_xyzw(-0.408_218, -0.234_57, -0.109_382, 0.875_426).normalize();
    }
    let mut l = scene.world.light_mut(id).unwrap();
    authoring::light::set_color(&mut l, Vec3::new(1.0, 0.95, 0.85));
    authoring::light::set_intensity(&mut l, 1.2);
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
