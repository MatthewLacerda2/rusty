//! A play-mode `Scene.Load` in the windowed loop (#2122): the tick that swapped the
//! scene ends its frame, the load's time is not owed, and no pose blends across it.

use super::frame_tests::playing_world;
use super::*;
use crate::core::paths;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Scene, ScriptComponent};

/// A level whose one script moves `Survivor` 1 m up on `Start` (a teleport the
/// blend would smear: under [`crate::render::interpolate::JUMP_DISTANCE`]).
fn write_level(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_2122_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let lua = paths::to_slash(&dir.join("mover.lua"));
    let code = "return { Start = function(id)\n\
        local s = Scene.FindEntityByName('Survivor')\n\
        local x, y, z = Transform.GetPosition(s)\n\
        Transform.SetPosition(s, x, y + 1, z) end }";
    std::fs::write(&lua, code).unwrap();
    let mut level = Scene::new();
    let id = level.add_entity("Mover".to_string());
    *level.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: lua,
        ..Default::default()
    }];
    let path = paths::to_slash(&dir.join("level.scene"));
    level.save_to_file(&path).unwrap();
    path
}

/// A world about to enter Play with one box, `Survivor`.
fn survivor() -> (GameWorld, u32) {
    let game = playing_world();
    let id = create_entity(
        &mut game.scene().borrow_mut(),
        "Survivor",
        Some(Primitive::Box),
    );
    (game, id)
}

/// In Play, past the entry frame, with `Survivor` marked
/// `DontDestroyOnLoad` and a load of `level` queued for the next tick.
fn loading(tag: &str) -> (GameWorld, u32, FixedTicks, DrawPoses) {
    let level = write_level(tag);
    let (mut game, id) = survivor();
    let (mut ticks, mut poses) = Default::default();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    for _ in 0..3 {
        advance_sim(
            &mut game,
            Step::of(&mut ticks, &mut poses),
            FIXED_DELTA_TIME,
        );
    }
    let eval = |code: String| game.script_manager().eval(&code).unwrap();
    eval(format!("Scene.DontDestroyOnLoad({id})"));
    eval(format!("Scene.Load('{level}')"));
    (game, id, ticks, poses)
}

#[test]
fn the_tick_that_loads_ends_its_frame_and_the_load_is_not_owed() {
    let (mut game, _, mut ticks, mut poses) = loading("owe");
    let (before, frame) = (game.scene().borrow().id(), game.play_frame());
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.1); // six ticks owed
    assert_ne!(game.scene().borrow().id(), before, "the level loaded");
    assert_eq!(game.play_frame(), frame + 1, "no tick after the load's");
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.3); // the load's cost
    assert_eq!(game.play_frame(), frame + 1, "the load's time is not owed");
    advance_sim(
        &mut game,
        Step::of(&mut ticks, &mut poses),
        FIXED_DELTA_TIME,
    );
    assert_eq!(game.play_frame(), frame + 2, "then the clock runs again");
}

#[test]
fn a_survivor_the_new_scene_moves_is_never_blended_from_its_old_pose() {
    let (mut game, id, mut ticks, mut poses) = loading("pose");
    // One tick a frame: the load's, then (on `main`) the level's `Start` in the next,
    // blended from the pose the survivor had in the outgoing scene.
    let mut frames = vec![FIXED_DELTA_TIME; 3];
    frames.extend([0.007; 6]);
    for raw in frames {
        let y = |g: &GameWorld| g.scene().borrow().world.transform(id).unwrap().position.y;
        let before = y(&game);
        advance_sim(&mut game, Step::of(&mut ticks, &mut poses), raw);
        if y(&game) - before > 0.5 {
            // The tick that ran the level's `Start`: drawn where it landed.
            let scene = game.scene().borrow();
            let drawn = poses.world(&scene, id).w_axis.truncate();
            let live = scene.world.world_position(id);
            assert_eq!(drawn, live, "not smeared up from the outgoing scene's pose");
            return;
        }
    }
    panic!(
        "Start never moved the survivor: {:?}",
        game.console().borrow().messages
    );
}

#[test]
fn a_frame_without_a_load_still_runs_every_owed_tick() {
    let (mut game, _) = survivor();
    let (mut ticks, mut poses) = Default::default();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    let frame = game.play_frame();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.1);
    assert_eq!(game.play_frame(), frame + 6);
}
