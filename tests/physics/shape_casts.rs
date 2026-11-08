//! The shape casts (#1881): `CapsuleCast`, `BoxCast` and `SphereCastAll`, each
//! with a hit and a miss. The oriented volumes and the argument errors are in
//! `shape_volumes`, over the same corridor.
//!
//! The corridor: two solid 2 m boxes on the +z axis, `near` centred at z = 10
//! (faces 9 and 11, x and y within ±1) and `far` at z = 20 (faces 19 and 21).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use crate::play::{playing, run};

/// A static 2 m box centred at `z` on the axis.
fn static_box(scene: &mut Scene, z: f32) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, z);
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        shape: ColliderShape::Box {
            size: Vec3::splat(2.0),
        },
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// The corridor in Play, one tick in: (`game`, near, far).
pub(super) fn corridor() -> (GameWorld, u32, u32) {
    let mut s = Scene::new();
    let near = static_box(&mut s, 10.0);
    let far = static_box(&mut s, 20.0);
    let mut game = playing(Rc::new(RefCell::new(s)), 20.0);
    run(&mut game, 1);
    (game, near, far)
}

/// Evaluate `line` in the game's script VM.
pub(super) fn eval(game: &GameWorld, line: &str) -> String {
    game.script_manager().eval(line).expect("evaluates")
}

/// `hit, id, distance` (distance to 2 decimals) of a `Physics.<call>`.
pub(super) fn first(game: &GameWorld, call: &str) -> String {
    let line = format!("local h, id, d = Physics.{call}; return h, id, string.format('%.2f', d)");
    eval(game, &line)
}

#[test]
fn capsule_cast_hits_a_body_in_its_path_and_misses_one_beside_it() {
    let (game, near, _) = corridor();
    // A standing body (centers at y 0 and 2, radius 0.4) walking +z.
    let walk = |x: f32| first(&game, &format!("CapsuleCast({x},0,0, {x},2,0, 0.4, 0,0,1)"));
    assert_eq!(walk(0.0), format!("true, {near}, 8.60"));
    assert_eq!(walk(2.0), "false, 0, 0.00", "1.6 m clear of the boxes");
    let past = "CapsuleCast(0,0,0, 0,2,0, 0.4, 0,0,1, nil, nil, 5)";
    assert_eq!(first(&game, past), "false, 0, 0.00", "max_distance holds");
    let ignored = format!("CapsuleCast(0,0,0, 0,2,0, 0.4, 0,0,1, {near})");
    assert!(
        first(&game, &ignored).starts_with("true, "),
        "ignore skips near"
    );
}

#[test]
fn box_cast_hits_and_misses_and_its_orientation_turns_the_box() {
    let (game, near, _) = corridor();
    assert_eq!(
        first(&game, "BoxCast(0,0,0, 0.5,0.5,0.5, 0,0,1)"),
        format!("true, {near}, 8.50")
    );
    // A plank 3 m long along z, held at x = 2.2, passes beside the boxes;
    // turned 90° about y it spans x 0.7..3.7 and strikes `near`.
    let plank = "BoxCast(2.2,0,0, 0.2,0.2,1.5, 0,0,1";
    assert_eq!(first(&game, &format!("{plank})")), "false, 0, 0.00");
    assert_eq!(
        first(&game, &format!("{plank}, 0,90,0)")),
        format!("true, {near}, 8.80")
    );
}

#[test]
fn sphere_cast_all_lists_every_hit_nearest_first() {
    let (game, near, far) = corridor();
    let all = |args: &str| {
        let line = format!(
            "local t = Physics.SphereCastAll({args}) local out = {{#t}} \
             for _, h in ipairs(t) do out[#out + 1] = h.id \
             out[#out + 1] = string.format('%.2f', h.distance) end \
             return table.concat(out, ' ')"
        );
        eval(&game, &line)
    };
    assert_eq!(
        all("0,0,0, 0,0,1, 0.5, 30"),
        format!("2 {near} 8.50 {far} 18.50")
    );
    assert_eq!(all("0,0,0, 0,0,1, 0.5, 15"), format!("1 {near} 8.50"));
    let unlimited = all("0,0,0, 0,0,1, 0.5, math.huge");
    assert_eq!(unlimited, format!("2 {near} 8.50 {far} 18.50"));
    assert_eq!(
        all("3,0,0, 0,0,1, 0.5, 30"),
        "0",
        "a sweep beside the boxes"
    );
    // A sweep that starts inside `near` lists it at distance 0, as SphereCast does.
    assert_eq!(
        all("0,0,10, 0,0,1, 0.5, 30"),
        format!("2 {near} 0.00 {far} 8.50")
    );
    assert_eq!(
        first(&game, "SphereCast(0,0,10, 0,0,1, 0.5)"),
        format!("true, {near}, 0.00")
    );
    let fields = "local h = Physics.SphereCastAll(0,0,0, 0,0,1, 0.5, 30)[1] \
                  return h.point.z, h.normal.z, h.root, h.material";
    assert_eq!(eval(&game, fields), format!("9, -1, {near}, nil"));
}
