//! The windowed loop on the fixed tick (#1877): the harness's state, input seen by one
//! tick, drawing between the last two ticks.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use super::frame_tests::playing_world;
use super::*;
use crate::app::{TapPoint, TickTap};
use crate::scene::authoring::{create_entity, default_rigidbody, Primitive};

/// A world about to enter Play with one box falling from 10 m.
fn falling() -> (GameWorld, u32) {
    let game = playing_world();
    let id = {
        let mut scene = game.scene().borrow_mut();
        let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
        scene.world.set_rigidbody(id, Some(default_rigidbody()));
        id
    };
    (game, id)
}

fn height(game: &GameWorld, id: u32) -> f32 {
    game.scene()
        .borrow()
        .world
        .transform(id)
        .unwrap()
        .position
        .y
}

/// The windowed loop: one accumulator and one set of poses across `frames`.
fn windowed(game: &mut GameWorld, poses: &mut DrawPoses, frames: &[f32]) {
    let mut ticks = FixedTicks::default();
    for &raw in frames {
        advance_sim(game, Step::of(&mut ticks, poses), raw);
    }
}

#[test]
fn uneven_frames_reach_the_state_the_harness_reaches_in_as_many_ticks() {
    let (mut window, id) = falling();
    let frames = [0.007, 0.033, 0.5, 0.007, 0.007, 0.007, 0.016, 0.1];
    windowed(&mut window, &mut DrawPoses::default(), &frames);
    let (mut harness, same) = falling();
    // The harness's loop (`dev::harness`): one fixed tick per step.
    for _ in 0..window.play_frame() {
        harness.tick(FIXED_DELTA_TIME);
    }
    assert_eq!(window.play_frame(), harness.play_frame());
    assert!(height(&window, id) < 10.0, "the box fell");
    assert_eq!(
        height(&window, id).to_bits(),
        height(&harness, same).to_bits()
    );
}

#[test]
fn a_hitch_runs_at_most_twenty_ticks() {
    let (mut game, _) = falling();
    let mut ticks = FixedTicks::default();
    let mut poses = DrawPoses::default();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016); // enter Play: one tick
    let entered = game.play_frame();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 2.0);
    assert_eq!(game.play_frame(), entered + 20);
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.005);
    assert_eq!(
        game.play_frame(),
        entered + 20,
        "a short frame after it owes none"
    );
}

#[test]
fn time_scale_zero_still_ticks_scripts_with_no_game_time() {
    let (mut game, id) = falling();
    let mut ticks = FixedTicks::default();
    let mut poses = DrawPoses::default();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    game.time().borrow_mut().set_time_scale(0.0);
    let (frame, y) = (game.play_frame(), height(&game, id));
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.05);
    assert_eq!(
        game.play_frame(),
        frame + 3,
        "a pause menu's Update keeps running"
    );
    assert_eq!(height(&game, id), y, "but nothing moves");
}

/// What each tick saw of a held key's edge and of the mouse delta.
#[derive(Default)]
struct Seen(Rc<RefCell<Vec<(bool, f64)>>>);

impl TickTap for Seen {
    fn at(&mut self, point: TapPoint, world: &mut GameWorld) {
        if point == TapPoint::AfterTick {
            let input = world.input().borrow();
            let seen = (input.get_key_down("W"), input.mouse_delta().0);
            self.0.borrow_mut().push(seen);
        }
    }
}

#[test]
fn input_reaches_the_first_tick_of_a_frame_and_waits_for_a_frame_with_none() {
    let (mut game, _) = falling();
    let mut ticks = FixedTicks::default();
    let mut poses = DrawPoses::default();
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.016);
    let log = Rc::new(RefCell::new(Vec::new()));
    game.set_tap(Some(Box::new(Seen(Rc::clone(&log)))));
    game.input().borrow_mut().press("W");
    game.input().borrow_mut().add_mouse_delta(4.0, 0.0);
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.005); // no tick owed
    assert!(log.borrow().is_empty(), "nothing ran");
    advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.05); // three ticks
    assert_eq!(*log.borrow(), vec![(true, 4.0), (false, 0.0), (false, 0.0)]);
}

#[test]
fn a_body_is_drawn_between_its_last_two_tick_positions() {
    let (mut game, id) = falling();
    let mut ticks = FixedTicks::default();
    let mut poses = DrawPoses::default();
    let mut checked = 0;
    for frame in 0..40 {
        let before = height(&game, id);
        let frames = game.play_frame();
        advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 0.007);
        if frame < 2 || game.play_frame() != frames + 1 || ticks.alpha() >= 1.0 {
            continue;
        }
        let after = height(&game, id);
        let drawn = poses.world(&game.scene().borrow(), id).w_axis.y;
        assert!(
            drawn <= before && drawn >= after,
            "{before} ≥ {drawn} ≥ {after}"
        );
        assert!(drawn > after, "blended, not at the tick");
        checked += 1;
    }
    assert!(checked > 5);
}
