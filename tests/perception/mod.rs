//! `AIPerception` end to end (#1900): sight through the game loop raises
//! `OnPerceptionUpdated` once on gain, a shot from behind is sensed at the
//! instigator, noises are heard and counted, and the settings round-trip
//! through the shared ops.

mod surface;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{AIPerceptionComponent, ColliderComponent, ColliderShape};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

/// A perceiver script logging each update in the global `LOG`.
const LOGGER: &str = r#"
local M = {}
function M.OnPerceptionUpdated(id, target, info)
  LOG = LOG or {}
  LOG[#LOG + 1] = { id = id, target = target, sense = info.sense, sensed = info.sensed,
    forgotten = info.forgotten or false, z = info.z, tag = info.tag }
end
function M.OnTakeDamage(id, amount, event) return amount end
return M
"#;

/// A bot at the origin facing +Z with the logger, and a player body at `player`.
pub fn arena(player: Vec3) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let bot = scene.add_entity("Bot".to_string());
    scene
        .world
        .set_ai_perception(bot, Some(AIPerceptionComponent::default()));
    attach_script(&mut scene, bot, "logger.lua", LOGGER);
    let target = scene.add_entity("Player".to_string());
    scene.world.transform_mut(target).unwrap().position = player;
    let collider = ColliderComponent {
        shape: ColliderShape::Box {
            size: Vec3::new(0.5, 1.8, 0.5),
        },
        ..default_collider()
    };
    scene.world.set_collider(target, Some(collider));
    (scene, bot, target)
}

/// `scene` in Play, `ticks` ticks in.
pub fn play(scene: Scene, ticks: usize) -> GameWorld {
    let mut game = playing(Rc::new(RefCell::new(scene)), 30.0);
    run(&mut game, ticks);
    game
}

/// Evaluate `line` in `game`'s gameplay VM.
pub fn eval(game: &GameWorld, line: &str) -> String {
    game.script_manager().eval(line).expect("evaluates")
}

#[test]
fn a_seen_player_fires_one_gain_and_is_known_where_it_stands() {
    let (scene, bot, player) = arena(Vec3::new(0.0, 1.0, 8.0));
    let mut game = play(scene, 3);
    let line = format!(
        "local k = AIPerception.Get({bot}, {player}) \
         return #LOG, LOG[1].target, LOG[1].sense, LOG[1].sensed, \
           AIPerception.CanSee({bot}, {player}), k.sense, k.z"
    );
    assert_eq!(
        eval(&game, &line),
        format!("1, {player}, sight, true, true, sight, 8")
    );
    // Behind the bot now: lost once, still known where it was seen.
    game.scene()
        .borrow_mut()
        .world
        .transform_mut(player)
        .unwrap()
        .position = Vec3::new(0.0, 1.0, -8.0);
    run(&mut game, 2);
    let line = format!(
        "return #LOG, LOG[2].sensed, AIPerception.CanSee({bot}, {player}), \
           AIPerception.Get({bot}, {player}).z"
    );
    assert_eq!(eval(&game, &line), "2, false, false, 8");
}

#[test]
fn a_shot_from_behind_is_sensed_at_the_instigator() {
    let (mut scene, bot, player) = arena(Vec3::new(0.0, 1.0, -6.0));
    let body = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..default_collider()
    };
    scene.world.set_collider(bot, Some(body));
    let mut game = play(scene, 2);
    let shot = format!(
        "local hits = Physics.RaycastAll(0,0.5,-5, 0,0,1, 20) \
         local first = hits[1].id == {player} and hits[2] or hits[1] \
         Damage.ApplyPoint(first, 25, 0,0,1, {{ type = 'bullet', instigator = {player} }}) \
         local k = AIPerception.Get({bot}, {player}) \
         return k.sense, k.z, k.amount, k.tag, k.sensed, AIPerception.CanSee({bot}, {player})"
    );
    assert_eq!(eval(&game, &shot), "damage, -6, 25, bullet, true, false");
    run(&mut game, 1);
    let line = "return #LOG, LOG[1].sense, LOG[1].tag";
    assert_eq!(
        eval(&game, line),
        "1, damage, bullet",
        "dispatched on the next tick"
    );
}
