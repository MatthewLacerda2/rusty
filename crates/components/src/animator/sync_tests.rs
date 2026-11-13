//! A non-looping follower's re-match rule (#1896): there, except that a
//! leader wrapping round its seam sends it on to its end, and its end holds.

use super::playback::Playback;
use super::playhead::{Playhead, Step};
use super::sync::follow_clamped;

#[test]
fn a_small_jump_back_is_a_re_match_back() {
    assert_eq!(follow_clamped(0.4, 0.3, 1.0, true), 0.3);
    assert_eq!(follow_clamped(0.4, 0.5, 1.0, false), 0.5);
}

#[test]
fn a_jump_back_over_half_the_length_is_the_leader_wrapping() {
    assert_eq!(follow_clamped(0.9, 0.05, 1.0, true), 1.0);
    assert_eq!(follow_clamped(0.1, 0.95, 1.0, false), 0.0);
}

#[test]
fn a_follower_resting_on_its_end_holds_while_the_leader_plays_on() {
    assert_eq!(follow_clamped(1.0, 0.2, 1.0, true), 1.0);
    assert_eq!(follow_clamped(1.3, 0.9, 1.0, true), 1.3);
    // Playing backwards it leaves the end at once.
    assert_eq!(follow_clamped(1.0, 0.95, 1.0, false), 0.95);
}

#[test]
fn a_backward_follower_resting_on_its_start_holds() {
    assert_eq!(follow_clamped(0.0, 0.45, 1.0, false), 0.0);
    assert_eq!(follow_clamped(-0.2, 0.1, 1.0, false), -0.2);
    // Playing forwards it leaves the start at once.
    assert_eq!(follow_clamped(0.0, 0.05, 1.0, true), 0.05);
}

fn moved(from: f32, to: f32) -> Step {
    Step {
        span: (from, to),
        events: (from, to),
        end: false,
    }
}

#[test]
fn owed_travel_is_dropped_when_play_turns_round() {
    let mut playback = Playback {
        time: 0.5,
        loop_clip: true,
        ..Playback::default()
    };
    playback.follow(Playhead::clip(1.0), 0.4, true); // re-matched back 0.1
    assert!((playback.sync.rewind - 0.1).abs() < 1e-6);
    // A pause owes nothing new and forgets nothing.
    assert_eq!(playback.repay(moved(0.4, 0.4), false), moved(0.4, 0.4));
    assert!((playback.sync.rewind - 0.1).abs() < 1e-6);
    // Played backwards now: its first crossing fires, nothing is owed.
    assert_eq!(playback.repay(moved(0.4, 0.35), false), moved(0.4, 0.35));
    assert_eq!(playback.sync.rewind, 0.0);
    // Forwards again: nothing stays silenced.
    assert_eq!(playback.repay(moved(0.35, 0.45), true), moved(0.35, 0.45));
}

#[test]
fn owed_travel_is_repaid_along_the_direction_it_was_owed() {
    let mut playback = Playback {
        time: 0.5,
        loop_clip: true,
        ..Playback::default()
    };
    playback.follow(Playhead::clip(1.0), 0.6, false); // backwards, moved on 0.1
    assert!((playback.sync.rewind + 0.1).abs() < 1e-6);
    let step = playback.repay(moved(0.6, 0.45), false);
    assert!((step.events.0 - 0.5).abs() < 1e-6, "{step:?}");
    assert_eq!(playback.sync.rewind, 0.0);
}

#[test]
fn forward_targets_are_followed() {
    assert_eq!(follow_clamped(0.2, 0.35, 1.0, true), 0.35);
}
