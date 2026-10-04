//! The fy_pool_day layout's promises (#747): the built geometry matches `layout`,
//! the water never collides, and the spawn-to-spawn walk stays open.

use glam::Vec3;

use super::layout::*;
use super::*;
use crate::components::TransformComponent;

fn built() -> Scene {
    let mut scene = Scene::new();
    build(&mut scene, BOT_SCRIPT);
    scene
}

fn transform(scene: &Scene, name: &str) -> TransformComponent {
    let id = scene.find_entity_by_name(name).expect(name);
    scene.world.transform(id).unwrap().clone()
}

/// Each ramp's top runs from the deck edge to the floor: both its top corners on
/// the spawn axis land where `ground_at` says the walkable surface is.
#[test]
fn the_ramps_join_the_deck_to_the_pool_floor() {
    let scene = built();
    for name in ["Ramp_South", "Ramp_North"] {
        let t = transform(&scene, name);
        let half = t.scale * 0.5;
        for along in [-1.0, 1.0] {
            let corner = t.position + t.rotation * Vec3::new(0.0, half.y, along * half.z);
            let want = ground_at(corner.x, corner.z).unwrap();
            assert!((corner.y - want).abs() < 1e-3, "{name}: {corner} vs {want}");
        }
    }
    assert!(ramp_grade() < crate::navigation::DEFAULT_MAX_SLOPE);
    assert!(
        ramp_grade().atan().to_degrees() < 45.0,
        "CharacterController limit"
    );
}

#[test]
fn the_water_is_seen_but_never_collides() {
    let scene = built();
    for name in ["Pool_Water", "Jacuzzi_Water"] {
        let id = scene.find_entity_by_name(name).unwrap();
        assert!(scene.world.collider(id).is_none(), "{name} has a collider");
        let key = &scene.world.material(id).unwrap().material;
        let water = &scene.materials[key];
        assert_eq!(water.render_mode, crate::scene::RenderMode::Transparent);
        assert!(water.alpha < 1.0);
    }
}

/// The jacuzzi's rim is higher than any step: getting in takes a jump.
#[test]
fn the_jacuzzi_rim_needs_a_jump() {
    let scene = built();
    let rim = transform(&scene, "Jacuzzi_Rim_East");
    let top = rim.position.y + rim.scale.y * 0.5;
    let player = scene.find_entity_by_name("Player").unwrap();
    let cc = scene.world.character_controller(player).unwrap();
    assert!(top - DECK_TOP > cc.step_offset * 1.6, "a step climbs it");
    assert!(top - BASIN_TOP > crate::navigation::DEFAULT_MAX_STEP);
}

/// The bot-player walks straight at Enemy_1: along that line nothing standing on the
/// deck but the enemy's cover crate comes within the Player's radius, until 4 m from
/// the enemy; the ramps carry it in and out of the pool. The cover crate does cross
/// the line, inside those 4 m, so the enemy's first chase paths round it.
#[test]
fn the_spawn_line_is_open_and_the_enemy_starts_behind_cover() {
    let scene = built();
    let (player, enemy) = (Vec3::from(PLAYER_SPAWN), Vec3::from(ENEMY_SPAWN));
    let walk = player.distance(enemy) - 4.0;
    let step = |d: f32| player + (enemy - player).normalize() * d;
    let blocks = |t: &TransformComponent, p: Vec3, pad: f32| {
        let half = t.scale * 0.5 + Vec3::splat(pad);
        (p.x - t.position.x).abs() < half.x && (p.z - t.position.z).abs() < half.z
    };
    let cover_id = scene.find_entity_by_name("Crate_North_Cover").unwrap();
    let cover = transform(&scene, "Crate_North_Cover");
    let standing: Vec<_> = (scene.world.ids_with_collider().into_iter())
        .filter(|&id| scene.world.is_static(id) && id != cover_id)
        .map(|id| scene.world.transform(id).unwrap().clone())
        .filter(|t| t.position.y + t.scale.y * 0.5 > DECK_TOP + 0.05)
        .collect();
    for i in 0..=100 {
        let p = step(walk * i as f32 / 100.0);
        assert!(ground_at(p.x, p.z).is_some(), "left the yard at {p}");
        for t in &standing {
            assert!(!blocks(t, p, 0.5), "{t:?} blocks the walk at {p}");
        }
        let on_ramp = p.z.abs() > POOL_HALF.1 - RAMP_RUN && p.z.abs() < POOL_HALF.1;
        let width = RAMP_HALF_WIDTH - 0.5;
        assert!(!on_ramp || p.x.abs() < width, "off the ramp at {p}");
    }
    let near = |i: i32| step(walk + 4.0 * i as f32 / 100.0);
    assert!(
        (0..=100).any(|i| blocks(&cover, near(i), 0.0)),
        "the cover crate is off the enemy's line"
    );
}
