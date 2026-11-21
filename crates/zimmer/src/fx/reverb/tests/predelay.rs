//! The pre-delay: the room answering late, by exactly the time asked.

use super::*;

/// Where the first non-silent sample of a channel is.
fn first(channel: &[f32]) -> usize {
    channel
        .iter()
        .position(|sample| sample.abs() > 1e-9)
        .expect("the room answered")
}

/// The first reflection lands the pre-delay later than it did without one,
/// on both sides, and is otherwise the same room: the tail is the one Freeverb
/// always made, moved.
#[test]
fn the_room_answers_the_predelay_later_and_is_the_same_room() {
    let plain = rung(44_100, 0.7, 0.4, 1.0);
    let mut late = impulse(44_100);
    apply(&mut late, 0.7, 0.4, 0.05, 1.0, 44_100.0);
    let gap = 2_205;
    assert_eq!(first(&late.l), first(&plain.l) + gap);
    assert_eq!(first(&late.r), first(&plain.r) + gap);
    assert_eq!(late.l[gap..], plain.l[..44_100 - gap]);
    assert_eq!(late.r[gap..], plain.r[..44_100 - gap]);
}

/// The dry sound arrives alone: at half wet, everything before the room's
/// first answer is the input at half level and nothing else.
#[test]
fn the_dry_sound_is_heard_alone_before_the_room() {
    let mut buf = impulse(4_410);
    apply(&mut buf, 0.7, 0.4, 0.05, 0.5, 44_100.0);
    assert_eq!(buf.l[0], 0.5);
    assert!(buf.l[1..first(&buf.l[1..]) + 1].iter().all(|s| *s == 0.0));
    assert!(first(&buf.l[1..]) + 1 >= 2_205 + COMB_LENGTHS[0]);
}

/// A pre-delay of nothing is no line at all, and one that is not a number, or
/// one past the cap, is the nearest one the room can take.
#[test]
fn a_predelay_is_clamped_and_none_is_the_room_as_it_was() {
    let mut zero = impulse(4_410);
    apply(&mut zero, 0.7, 0.4, 0.0, 1.0, 44_100.0);
    assert_eq!(zero, rung(4_410, 0.7, 0.4, 1.0));
    assert_eq!(predelay_seconds(f32::NAN), 0.0);
    assert_eq!(predelay_seconds(-1.0), 0.0);
    assert_eq!(predelay_seconds(9.0), MAX_PREDELAY);
    assert_eq!(predelay_seconds(0.03), 0.03);
}
