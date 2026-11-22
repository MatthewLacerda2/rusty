pub mod layout;
mod looks;
mod pool;
mod seed;
mod yard;

use glam::{Quat, Vec3};

use crate::authoring::{
    self, add_component, ai::perception, animator, create_drawn_entity, create_entity,
    nav::agent as nav_agent, physics::character_controller as cc, physics::collider, ComponentKind,
    Primitive,
};
use rusty_scene::{ColliderShape, MaterialComponent, Scene};

pub use looks::{checker_recipe, shader_recipe, CHECKER_MAP, SHADER_NAME};
pub use seed::{seed_default_assets, seed_default_assets_into};

pub const PLAYER_CONTROLLER_SCRIPT: &str = "assets/scripts/player_controller.lua";
pub const BOT_SCRIPT: &str = "assets/scripts/bot.lua";
/// Centre to centre: the box's 0.92 m half-diagonal plus the Player's 0.5 m radius, with room
/// to spare.
const ENEMY_STOPPING_DISTANCE: f32 = 1.5;
/// The bots' sight looks only for it, so 30 bots trace 30 pairs, not 870. Unity's first user
/// layer.
pub const PLAYER_LAYER: u8 = 8;
/// Above the 2 m box's centre: 0.2 m under its top, 1.8 m off the ground.
const ENEMY_EYE_HEIGHT: f32 = 0.8;

/// The call order fixes the ids: Pool_Floor 1, Player 2, ramps 3 and 4, Enemy_1 5, Sun 6.
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

fn block(scene: &mut Scene, name: &str, min: Vec3, max: Vec3, material: &str) -> u32 {
    let at = ((min + max) * 0.5, max - min);
    let unit = ColliderShape::Box { size: Vec3::ONE };
    solid(scene, name, Primitive::Box, at, unit, material)
}

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
    if let Some(mut c) = scene.world.collider_mut(id) {
        collider::set_shape(&mut c, shape);
    }
    wear(scene, id, material);
    id
}

fn place(scene: &mut Scene, id: u32, pos: Vec3, scale: Vec3) {
    let Some(mut t) = scene.world.transform_mut(id) else {
        return;
    };
    t.position = pos;
    t.scale = scale;
}

fn wear(scene: &mut Scene, id: u32, key: &str) {
    let material = key.to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(material)));
}

fn attach_script(scene: &mut Scene, id: u32, path: &str) {
    authoring::add_script(&mut scene.world, id, path, Default::default());
}

/// `height` and `radius` before the transform's scale.
fn character(scene: &mut Scene, id: u32, height: f32, radius: f32) {
    add_component(scene, id, ComponentKind::CharacterController);
    let Some(mut c) = scene.world.character_controller_mut(id) else {
        return;
    };
    cc::set_height(&mut c, height);
    cc::set_radius(&mut c, radius);
}

fn add_player(scene: &mut Scene) {
    let id = create_drawn_entity(scene, "Player", Some(Primitive::Cylinder));
    place(
        scene,
        id,
        Vec3::from(layout::PLAYER_SPAWN),
        Vec3::new(1.0, 0.8, 1.0),
    );
    character(scene, id, 2.0, 0.5);
    scene.layers.set_name(PLAYER_LAYER, "Player");
    scene.world.set_layer(id, PLAYER_LAYER);
    wear(scene, id, looks::PLAYER);
    attach_script(scene, id, PLAYER_CONTROLLER_SCRIPT);
}

fn add_enemy(scene: &mut Scene, bot_script: &str) {
    let id = create_drawn_entity(scene, "Enemy_1", Some(Primitive::Box));
    place(
        scene,
        id,
        Vec3::from(layout::ENEMY_SPAWN),
        Vec3::new(1.3, 2.0, 1.3),
    );
    character(scene, id, 1.0, 0.5);
    add_component(scene, id, ComponentKind::Animator);
    if let Some(mut a) = scene.world.animator_mut(id) {
        animator::set_clip(&mut a, "Walk".to_string());
        animator::set_speed(&mut a, 3.0);
    }
    add_component(scene, id, ComponentKind::NavMeshAgent);
    if let Some(mut a) = scene.world.nav_agent_mut(id) {
        nav_agent::set_active(&mut a, true);
        nav_agent::set_radius(&mut a, 0.65);
        nav_agent::set_speed(&mut a, 3.5);
        nav_agent::set_acceleration(&mut a, 8.0);
        nav_agent::set_stopping_distance(&mut a, ENEMY_STOPPING_DISTANCE);
        // The box is centred on its origin, 1 m above its feet.
        nav_agent::set_base_offset(&mut a, 1.0);
        nav_agent::set_target(&mut a, Vec3::ZERO);
    }
    add_component(scene, id, ComponentKind::AIPerception);
    if let Some(mut p) = scene.world.ai_perception_mut(id) {
        perception::set_sight_mask(&mut p, 1 << PLAYER_LAYER);
        perception::set_eye_height(&mut p, ENEMY_EYE_HEIGHT);
    }
    wear(scene, id, looks::ENEMY);
    if !bot_script.is_empty() {
        attach_script(scene, id, bot_script);
    }
}

fn add_sun(scene: &mut Scene) {
    let id = create_entity(scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.set_static(id, true);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = Vec3::new(0.0, 12.0, 0.0);
        t.rotation = Quat::from_xyzw(-0.408_218, -0.234_57, -0.109_382, 0.875_426).normalize();
    }
    let Some(mut l) = scene.world.light_mut(id) else {
        return;
    };
    authoring::rendering::light::set_color(&mut l, Vec3::new(1.0, 0.95, 0.85));
    authoring::rendering::light::set_intensity(&mut l, 1.2);
    authoring::rendering::light::set_mode(&mut l, rusty_components::LightMode::Baked);
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
