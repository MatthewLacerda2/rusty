//! The cast options (#1720): `max_distance` and `query_triggers` on the single
//! casts, `query_triggers` on the multi-hit and volume queries, and
//! `RaycastAllNonAlloc` refilling the caller's table without allocating.
//!
//! The corridor: a 2 m trigger centred at z = 3 (faces 2 and 4) and a 2 m solid
//! wall centred at z = 10 (faces 9 and 11); every ray starts at the origin, +z.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use crate::play::{playing, run};

/// A static 2 m box centred at `z`, a trigger when `trigger`.
fn static_box(scene: &mut Scene, z: f32, trigger: bool) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, z);
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        shape: ColliderShape::Box {
            size: Vec3::splat(2.0),
        },
        is_trigger: trigger,
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// The corridor in Play, one tick in: (`game`, trigger, wall).
fn corridor() -> (GameWorld, u32, u32) {
    let mut s = Scene::new();
    let trigger = static_box(&mut s, 3.0, true);
    let wall = static_box(&mut s, 10.0, false);
    let mut game = playing(Rc::new(RefCell::new(s)), 20.0);
    run(&mut game, 1);
    (game, trigger, wall)
}

/// `hit, id, distance` of a `Physics.<verb>(<args>)` cast.
fn cast(game: &GameWorld, verb: &str, args: &str) -> String {
    let line = format!("local h, id, d = Physics.{verb}({args}); return h, id, d");
    game.script_manager().eval(&line).expect("evaluates")
}

#[test]
fn raycast_stops_at_max_distance_and_can_skip_triggers() {
    let (game, trigger, wall) = corridor();
    let ray = |tail: &str| cast(&game, "Raycast", &format!("0,0,0, 0,0,1{tail}"));
    assert_eq!(
        ray(""),
        format!("true, {trigger}, 2"),
        "triggers hit by default"
    );
    assert_eq!(
        ray(", nil, nil, 1"),
        "false, 0, 0",
        "the trigger is past 1 m"
    );
    assert_eq!(
        ray(", nil, nil, 5, false"),
        "false, 0, 0",
        "the wall is past 5 m"
    );
    assert_eq!(ray(", nil, nil, 20, false"), format!("true, {wall}, 9"));
    assert_eq!(
        ray(", nil, nil, math.huge, false"),
        format!("true, {wall}, 9")
    );
    assert_eq!(ray(", nil, nil, nil, true"), format!("true, {trigger}, 2"));
    // Unity: a negative or NaN distance casts nothing.
    assert_eq!(ray(", nil, nil, -1"), "false, 0, 0");
    assert_eq!(ray(", nil, nil, 0/0"), "false, 0, 0");
    assert_eq!(
        ray(&format!(", {trigger}, nil, 9.5")),
        format!("true, {wall}, 9")
    );
}

#[test]
fn sphere_cast_takes_the_same_trailing_options() {
    let (game, trigger, wall) = corridor();
    let sweep = |tail: &str| cast(&game, "SphereCast", &format!("0,0,0, 0,0,1, 0.5{tail}"));
    assert!(sweep("").starts_with(&format!("true, {trigger}, ")));
    assert_eq!(sweep(", nil, nil, 1"), "false, 0, 0");
    assert_eq!(sweep(", nil, nil, 8, false"), "false, 0, 0");
    assert!(sweep(", nil, nil, 9, false").starts_with(&format!("true, {wall}, ")));
}

#[test]
fn multi_hit_and_volume_queries_skip_triggers_on_request() {
    let (game, trigger, wall) = corridor();
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    let all = "local t = Physics.RaycastAll(0,0,0, 0,0,1, 20, nil, ARG) return #t, t[1].id";
    assert_eq!(eval(&all.replace("ARG", "nil")), format!("2, {trigger}"));
    assert_eq!(eval(&all.replace("ARG", "false")), format!("1, {wall}"));
    let through = "local t = Physics.RaycastThrough(0,0,0, 0,0,1, 20, nil, false) return #t";
    assert_eq!(eval(through), "1");
    assert_eq!(eval("#Physics.RaycastAll(0,0,0, 0,0,1, -1)"), "0");
    let near = "#Physics.OverlapSphere(0,0,3, 1.5, nil, false), \
                #Physics.OverlapBox(0,0,3, 1,1,1, nil,nil,nil, nil, false), \
                #Physics.OverlapCapsule(0,0,2, 0,0,4, 0.5, nil, false), \
                Physics.CheckSphere(0,0,3, 1.5), Physics.CheckBox(0,0,3, 1,1,1, nil,nil,nil, nil, false)";
    assert_eq!(eval(&format!("return {near}")), "0, 0, 0, true, false");
}

#[test]
fn non_alloc_refills_the_callers_entries_and_returns_the_count() {
    let (game, trigger, wall) = corridor();
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    eval("R = {}; N = Physics.RaycastAllNonAlloc(R, 0,0,0, 0,0,1, 20)");
    assert_eq!(
        eval("return N, #R, R[1].id, R[2].id, R[2].distance, R[2].point.z"),
        format!("2, 2, {trigger}, {wall}, 9, 9"),
        "an empty table grows to the hits"
    );
    eval("E1, P1, KEEP = R[1], R[1].point, {} R[3] = KEEP");
    let again = "return Physics.RaycastAllNonAlloc(R, 0,0,0, 0,0,1, 20, nil, false), \
                 R[1] == E1, R[1].point == P1, R[1].id, R[2].id, R[3] == KEEP";
    assert_eq!(
        eval(again),
        format!("1, true, true, {wall}, {wall}, true"),
        "entry tables are reused; slots past the count stay as they were"
    );
}

#[test]
fn non_alloc_allocates_nothing_on_a_reused_table() {
    let (game, _, _) = corridor();
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    let growth = |call: &str| -> f64 {
        let script = format!(
            "local R = {{}} {call} collectgarbage('collect') collectgarbage('stop') \
             local before = collectgarbage('count') \
             for _ = 1, 1000 do {call} end \
             local grown = collectgarbage('count') - before \
             collectgarbage('restart') return grown"
        );
        eval(&script).parse().expect("a number")
    };
    let fresh = growth("local t = Physics.RaycastAll(0,0,0, 0,0,1, 20)");
    let reused = growth("Physics.RaycastAllNonAlloc(R, 0,0,0, 0,0,1, 20)");
    assert!(fresh > 100.0, "RaycastAll allocates its tables: {fresh} KB");
    assert!(reused < 1.0, "1000 reused calls grew the heap {reused} KB");
}
