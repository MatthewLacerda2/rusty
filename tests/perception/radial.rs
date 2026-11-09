//! `Damage.ApplyRadial`'s damage sense (#1900): sensed from the struck body,
//! once per perceiver per blast, with the largest amount any of its colliders
//! took.

use glam::Vec3;
use rusty::components::{AIPerceptionComponent, ColliderComponent, ColliderShape};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use super::{eval, play};
use crate::play::attach_script;

/// A perceiver script logging each update's sense and amount in `LOG`.
const SENSES: &str = r#"
local M = {}
function M.OnPerceptionUpdated(id, target, info)
  LOG = LOG or {}
  LOG[#LOG + 1] = string.format("%d:%s:%g", id, info.sense, info.amount)
end
return M
"#;

/// A health script: takes every hit.
const HEALTH: &str = "local M = {} function M.OnTakeDamage(id, a) return a end return M";

/// 100 damage at the origin, falling linearly to nothing at 10 m.
const BLAST: &str = "Damage.ApplyRadial(0,0,0, 100, { outer = 10 })";

/// A 0.5 m box collider on a new child of `parent` at local `at`.
fn hitbox(scene: &mut Scene, parent: u32, at: Vec3) -> u32 {
    let id = scene.add_entity("hitbox".to_string());
    scene.set_parent(id, Some(parent)).unwrap();
    scene.world.transform_mut(id).unwrap().position = at;
    let c = ColliderComponent {
        shape: ColliderShape::Box {
            size: Vec3::splat(0.5),
        },
        ..default_collider()
    };
    scene.world.set_collider(id, Some(c));
    id
}

/// One update at the time of the call, then none more.
fn one_report(scene: Scene) -> String {
    let mut game = play(scene, 1);
    eval(&game, BLAST);
    crate::play::run(&mut game, 1);
    eval(&game, "return #LOG, LOG[1]")
}

#[test]
fn a_bot_of_many_hitboxes_and_no_health_senses_one_blast_once() {
    let mut scene = Scene::new();
    let bot = scene.add_entity("Bot".to_string());
    scene.world.transform_mut(bot).unwrap().position = Vec3::new(0.0, 0.0, 4.0);
    let p = AIPerceptionComponent::default();
    scene.world.set_ai_perception(bot, Some(p));
    attach_script(&mut scene, bot, "senses.lua", SENSES);
    // At 4 m, 5 m and 6 m: the nearest took 60, the others 50 and 40.
    for z in [0.0, 1.0, 2.0] {
        hitbox(&mut scene, bot, Vec3::new(0.0, 0.0, z + 0.25));
    }
    assert_eq!(
        one_report(scene),
        format!("1, {bot}:damage:60"),
        "one report, the largest amount"
    );
}

#[test]
fn a_perceiver_under_a_health_script_senses_the_blast() {
    let mut scene = Scene::new();
    let body = scene.add_entity("Body".to_string());
    scene.world.transform_mut(body).unwrap().position = Vec3::new(0.0, 0.0, 4.0);
    attach_script(&mut scene, body, "health.lua", HEALTH);
    let head = scene.add_entity("Head".to_string());
    scene.set_parent(head, Some(body)).unwrap();
    let p = AIPerceptionComponent::default();
    scene.world.set_ai_perception(head, Some(p));
    attach_script(&mut scene, head, "senses.lua", SENSES);
    hitbox(&mut scene, head, Vec3::new(0.0, 0.0, 0.25));
    assert_eq!(one_report(scene), format!("1, {head}:damage:60"));
}
