//! `Damage.ApplyRadial`: Unreal's falloff, cover behind a wall, a many-hitbox
//! character damaged once, and the same list on every run.

use glam::Vec3;
use rusty::scene::Scene;

use super::{eval, play, static_box, RECORDER};
use crate::play::attach_script;

/// 100 damage at the origin: full inside 1 m, none past 5 m, linear.
const BLAST: &str = "Damage.ApplyRadial(0,0,0, 100, { inner = 1, outer = 5 })";

/// The result list as `id:amount:distance` entries.
fn listed(call: &str) -> String {
    format!(
        "local out = {{}} for _, r in ipairs({call}) do \
         out[#out + 1] = string.format('%d:%g:%g', r.id, r.amount, r.distance) end \
         return table.concat(out, ' ')"
    )
}

/// Targets at 0.25 m (inside, +x), 3 m (halfway out, -z), 6 m (past it, +x) and
/// one 3 m behind a wall (+z); returns the game and their ids.
fn range() -> (rusty::app::GameWorld, [u32; 4]) {
    let mut s = Scene::new();
    let half = Vec3::splat(0.5);
    let near = static_box(&mut s, Vec3::X * 0.5, half, Some(RECORDER));
    let mid = static_box(&mut s, Vec3::Z * -3.25, half, Some(RECORDER));
    let far = static_box(&mut s, Vec3::X * 6.25, half, Some(RECORDER));
    let hidden = static_box(&mut s, Vec3::Z * 3.25, half, Some(RECORDER));
    static_box(&mut s, Vec3::Z * 1.5, Vec3::new(2.0, 2.0, 0.2), None);
    (play(s), [near, mid, far, hidden])
}

#[test]
fn the_falloff_runs_from_inner_to_outer_and_a_wall_shields() {
    let (game, [near, mid, _far, _hidden]) = range();
    assert_eq!(
        eval(&game, &listed(BLAST)),
        format!("{near}:100:0.25 {mid}:50:3"),
        "full inside inner, half halfway; past outer and behind the wall nothing"
    );
    assert_eq!(
        eval(&game, "return #LOG, LOG[1].kind, LOG[2].distance"),
        "2, radial, 3"
    );
}

#[test]
fn the_falloff_exponent_and_min_shape_the_drop() {
    let (game, [near, mid, _, _]) = range();
    let call = "Damage.ApplyRadial(0,0,0, 100, { inner = 1, outer = 5, min = 20, falloff = 2 })";
    // t = 0.5: 20 + 80 * 0.5^2.
    assert_eq!(
        eval(&game, &listed(call)),
        format!("{near}:100:0.25 {mid}:40:3")
    );
}

#[test]
fn a_character_with_many_hitboxes_is_damaged_once() {
    let mut scene = Scene::new();
    let rig = crate::hitboxes::rig::character(&mut scene, Vec3::Z * 1.0, false);
    attach_script(&mut scene, rig, "rig.lua", super::RECORDER);
    let game = play(scene);
    let call = "Damage.ApplyRadial(0,1.5,0, 100, { outer = 4 })";
    let line = format!("local r = {call} return #r, r[1].id, #LOG");
    assert_eq!(eval(&game, &line), format!("1, {rig}, 1"));
}

#[test]
fn bad_options_are_errors() {
    let (game, _) = range();
    let err = |opts: &str| {
        let call = format!("Damage.ApplyRadial(0,0,0, 10, {opts})");
        game.script_manager().eval(&call).expect_err(&call)
    };
    assert!(err("{}").contains("opts.outer is required"));
    assert!(err("{ inner = 3, outer = 2 }").contains("inner <= outer"));
    assert!(err("{ outer = 0/0 }").contains("finite"));
}

#[test]
fn two_runs_over_one_scene_list_the_same() {
    let first = eval(&range().0, &listed(BLAST));
    let second = eval(&range().0, &listed(BLAST));
    assert_eq!(first, second);
    assert!(!first.is_empty());
}
