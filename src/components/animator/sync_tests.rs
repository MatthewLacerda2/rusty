//! A non-looping follower's re-match rule (#1896): there, except that a
//! leader wrapping round its seam sends it on to its end, and its end holds.

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
fn forward_targets_are_followed() {
    assert_eq!(follow_clamped(0.2, 0.35, 1.0, true), 0.35);
}
