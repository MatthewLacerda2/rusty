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
//! The layout demonstrates what already works:
//! - Enemy_1 starts behind a cover wall, so its first chase paths **around** it;
//! - a crate and a metal sphere sit on the floor for contact shadows;
//! - a handful of material looks: the checkerboard floor, tinted-checker walls and
//!   crate, glossy plastic, polished metal, and the enemy's baked rim shader.
//!
//! The straight line from the Player to Enemy_1 stays clear up to 4 m from the enemy:
//! the bot-player (`project/scripts/bot_player.lua`) walks it with no pathing.
//!
//! Submodules: `looks` (materials, the checker recipe, the default shader recipe),
//! `seed` (bakes those into `project/assets/` — the only I/O here).

mod looks;
mod seed;

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

/// Build the default scene into `scene`. `bot_script` is Enemy_1's Lua brain; empty
/// leaves it unscripted (the harness's default, so a test drives the enemy itself).
/// Pure: no I/O, no RNG — the maps and shader it names are seeded separately.
///
/// Ids are stable (Floor 1, Player 2, walls 3–4, Enemy_1 5, Sun 6, props 7–8).
pub fn build(scene: &mut Scene, bot_script: &str) {
    looks::define_all(&mut scene.materials);
    add_floor(scene);
    add_player(scene);
    add_walls(scene);
    add_enemy(scene, bot_script);
    add_sun(scene);
    add_props(scene);
    scene.update_all_colliders();
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

/// Floor (id 1): a 15 m plane scaled 2.5 — ±18.75 m, the nav tests' arena.
fn add_floor(scene: &mut Scene) {
    let size = Vec3::new(15.0, 0.1, 15.0);
    let at = (Vec3::ZERO, Vec3::new(2.5, 1.0, 2.5));
    let shape = ColliderShape::Box { size };
    solid(
        scene,
        "Floor_Plane",
        Primitive::Plane,
        at,
        shape,
        looks::FLOOR,
    );
}

/// Player (id 2): a 1.6 m × 0.5 m cylinder — the unit primitive scaled, its capsule
/// scaling with it.
fn add_player(scene: &mut Scene) {
    let id = create_entity(scene, "Player", Some(Primitive::Cylinder));
    place(
        scene,
        id,
        Vec3::new(0.0, 1.5, -6.0),
        Vec3::new(1.0, 1.6, 1.0),
    );
    character(scene, id, 1.0, 0.5);
    wear(scene, id, looks::PLAYER);
    attach_script(scene, id, PLAYER_CONTROLLER_SCRIPT);
}

/// Walls (ids 3, 4). The left one is Enemy_1's cover: it crosses the enemy's straight
/// line to the Player, so its first chase goes round an end.
fn add_walls(scene: &mut Scene) {
    let unit = || ColliderShape::Box { size: Vec3::ONE };
    let cover = (Vec3::new(7.5, 1.0, 6.0), Vec3::new(4.0, 2.0, 0.6));
    solid(
        scene,
        "Obstacle_Wall_Left",
        Primitive::Box,
        cover,
        unit(),
        looks::WALL,
    );
    let right = (Vec3::new(-3.0, 1.0, 4.0), Vec3::new(4.0, 2.0, 1.0));
    solid(
        scene,
        "Obstacle_Wall_Right",
        Primitive::Box,
        right,
        unit(),
        looks::WALL,
    );
}

/// Enemy_1 (id 5): a 2 m × 1.3 m box chasing the Player on the navmesh. Its body is
/// centred on its origin, so the agent stands it 1 m above its feet (#666).
fn add_enemy(scene: &mut Scene, bot_script: &str) {
    let id = create_entity(scene, "Enemy_1", Some(Primitive::Box));
    place(
        scene,
        id,
        Vec3::new(8.0, 1.0, 8.0),
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
        nav_agent::set_radius(&mut a, 0.65);
        nav_agent::set_speed(&mut a, 3.5);
        nav_agent::set_acceleration(&mut a, 8.0);
        nav_agent::set_stopping_distance(&mut a, 0.5);
        nav_agent::set_base_offset(&mut a, 1.0);
        nav_agent::set_target(&mut a, Vec3::ZERO);
    }
    wear(scene, id, looks::ENEMY);
    if !bot_script.is_empty() {
        attach_script(scene, id, bot_script);
    }
}

/// Sun (id 6): a warm directional light from high to one side, so every prop drops
/// a shadow across the checker.
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

/// Props (ids 7, 8): a crate and a polished sphere resting on the floor, off the
/// Player's and the enemy's routes.
fn add_props(scene: &mut Scene) {
    let crate_at = (Vec3::new(-6.0, 0.5, -1.0), Vec3::ONE);
    let unit = ColliderShape::Box { size: Vec3::ONE };
    solid(scene, "Crate", Primitive::Box, crate_at, unit, looks::CRATE);
    let ball_at = (Vec3::new(-3.5, 0.75, -2.5), Vec3::splat(0.75));
    let ball = ColliderShape::Sphere { radius: 1.0 };
    solid(
        scene,
        "Metal_Sphere",
        Primitive::Sphere,
        ball_at,
        ball,
        looks::METAL,
    );
}

#[cfg(test)]
mod tests;
