use glam::Vec3;

use super::looks::{self, FLOOR};
use super::*;
use crate::scene::authoring::material::MAPS_DIR;
use crate::scene::{apply_scene_data, to_scene_data};
use crate::shadergen::ShaderRecipe;

fn built() -> Scene {
    let mut scene = Scene::new();
    build(&mut scene, BOT_SCRIPT);
    scene
}

#[test]
fn the_cast_keeps_its_names_and_ids() {
    let scene = built();
    let cast = [
        ("Floor_Plane", 1),
        ("Player", 2),
        ("Obstacle_Wall_Left", 3),
        ("Obstacle_Wall_Right", 4),
        ("Enemy_1", 5),
        ("Sun", 6),
    ];
    for (name, id) in cast {
        assert_eq!(scene.find_entity_by_name(name), Some(id), "{name}");
    }
    assert!(scene.world.light(6).is_some(), "the sun lights the scene");
    assert_eq!(scene.world.nav_agent(5).unwrap().base_offset, 1.0, "#666");
}

#[test]
fn every_mesh_wears_a_defined_material() {
    let scene = built();
    for id in scene.world.ids_with_mesh() {
        let key = scene
            .world
            .material(id)
            .expect("has a material")
            .material
            .clone();
        assert!(
            scene.materials.contains_key(&key),
            "{key} is in the library"
        );
    }
    let enemy = &scene.materials[looks::ENEMY];
    assert_eq!(enemy.shader.as_deref(), Some(SHADER_NAME));
}

#[test]
fn the_floor_names_the_map_its_recipe_bakes() {
    assert_eq!(CHECKER_MAP, format!("{MAPS_DIR}/{FLOOR}_base_color.png"));
    let floor = &built().materials[FLOOR];
    assert_eq!(floor.base_color_map.as_deref(), Some(CHECKER_MAP));
    assert_eq!(floor.maps_recipe, Some(checker_recipe()));
}

#[test]
fn the_built_scene_survives_its_saved_document() {
    let scene = built();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, to_scene_data(&scene));
    assert_eq!(loaded.materials.len(), scene.materials.len());
    assert_eq!(loaded.materials[FLOOR].maps_recipe, Some(checker_recipe()));
    let enemy = loaded.find_entity_by_name("Enemy_1").unwrap();
    assert_eq!(
        loaded.world.transform(enemy).unwrap().scale,
        Vec3::new(1.3, 2.0, 1.3)
    );
}

/// Enemy_1's agent is authored ready to chase, and stops where neither body
/// overlaps the other even corner-on: half its box's diagonal plus the Player's
/// capsule radius stays inside the stopping distance (#743).
#[test]
fn the_enemy_stops_clear_of_the_player() {
    let scene = built();
    let (enemy, player) = (5, 2);
    let agent = scene.world.nav_agent(enemy).unwrap();
    assert!(agent.active, "authored active, not switched on by bot.lua");
    let box_half = scene.world.transform(enemy).unwrap().scale.x * 0.5;
    let p_scale = scene.world.transform(player).unwrap().scale;
    let p_radius = scene.world.character_controller(player).unwrap().radius * p_scale.x;
    let corner = box_half * std::f32::consts::SQRT_2;
    assert!(
        corner + p_radius < agent.stopping_distance,
        "{}",
        agent.stopping_distance
    );
}

/// The enemy's straight line to the Player crosses its cover wall, so its first
/// chase has to path round it.
#[test]
fn the_cover_wall_stands_between_enemy_and_player() {
    let scene = built();
    let at = |name| {
        let id = scene.find_entity_by_name(name).unwrap();
        scene.world.transform(id).unwrap().clone()
    };
    let (enemy, player, wall) = (at("Enemy_1"), at("Player"), at("Obstacle_Wall_Left"));
    let half = wall.scale * 0.5;
    let crosses = (0..=100).any(|i| {
        let p = enemy.position.lerp(player.position, i as f32 / 100.0);
        (p.x - wall.position.x).abs() < half.x && (p.z - wall.position.z).abs() < half.z
    });
    assert!(crosses, "the wall is off the enemy's line");
}

#[test]
fn seeding_bakes_the_texture_and_shader_once() {
    let dir = crate::test_temp::dir().join("rusty_667_seed");
    let (tex, shaders) = (dir.join("textures"), dir.join("shaders"));
    std::fs::remove_dir_all(&dir).ok();
    seed_default_assets_into(&tex, &shaders).expect("seeds");

    let png = tex.join(format!("{FLOOR}_base_color.png"));
    assert!(png.is_file(), "checker baked");
    for ext in ["wgsl", "params.json"] {
        assert!(
            shaders.join(format!("{SHADER_NAME}.{ext}")).is_file(),
            "{ext}"
        );
    }
    let json = std::fs::read_to_string(shaders.join(format!("{SHADER_NAME}.recipe.json")));
    assert_eq!(ShaderRecipe::from_json(&json.unwrap()), Ok(shader_recipe()));

    // An edited copy is the user's: a second seed leaves it alone.
    std::fs::write(&png, b"edited").unwrap();
    seed_default_assets_into(&tex, &shaders).expect("re-seeds");
    assert_eq!(std::fs::read(&png).unwrap(), b"edited");
    let left: Vec<_> = std::fs::read_dir(&tex).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "no staging directory left behind");
    std::fs::remove_dir_all(&dir).ok();
}
