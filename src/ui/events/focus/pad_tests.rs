//! Tests for gamepad focus navigation (#672): pad buttons and the stick map onto
//! Move / Submit / Cancel, a held Move repeats on sim time, and a stick short of
//! its press point does nothing.

use glam::Vec2;

use super::super::fixture::{button, scene, Handlers, Rig};
use super::super::UiHook::*;
use super::actions::{REPEAT_DELAY, REPEAT_RATE};
use super::{nav_actions, MoveRepeat, NavAction};

/// A row of three buttons, left to right.
fn row() -> (crate::scene::Scene, [u32; 3]) {
    let (mut s, canvas) = scene();
    let size = Vec2::splat(100.0);
    let a = button(&mut s, canvas, Vec2::new(0.0, 0.0), size);
    let b = button(&mut s, canvas, Vec2::new(300.0, 0.0), size);
    let c = button(&mut s, canvas, Vec2::new(600.0, 0.0), size);
    (s, [a, b, c])
}

#[test]
fn pad_buttons_map_to_logical_actions_on_any_pad() {
    let mut input = crate::core::input::InputState::new();
    for k in ["PADUP", "PAD2RIGHT", "PAD1A", "PADB", "Left", "PADLEFT"] {
        input.press(k);
    }
    input.begin_tick();
    let expected = [
        NavAction::Move(0),
        NavAction::Move(2),
        NavAction::Move(3),
        NavAction::Submit,
        NavAction::Cancel,
    ];
    assert_eq!(
        nav_actions(&input),
        expected,
        "arrow + d-pad left is one Move"
    );
}

#[test]
fn a_held_move_repeats_after_the_delay_then_at_the_rate() {
    let dt = 0.01;
    let mut r = MoveRepeat::default();
    assert_eq!(
        r.tick(Some(3), true, dt),
        None,
        "the press is the edge's Move"
    );
    let mut moves = Vec::new();
    // 1.0 s of holding after the press, in 0.01 s ticks.
    for i in 1..=100 {
        if r.tick(Some(3), false, dt).is_some() {
            moves.push(i);
        }
    }
    let at = |s: f32| (s / dt).round() as i32;
    let expected: Vec<i32> = (0..6)
        .map(|k| at(REPEAT_DELAY + k as f32 * REPEAT_RATE))
        .collect();
    assert_eq!(moves, expected, "t=0.5, then every 0.1 s");
    assert_eq!(r.tick(None, false, dt), None, "released");
    assert_eq!(
        r.tick(None, false, 10.0),
        None,
        "nothing held, nothing repeats"
    );
}

#[test]
fn a_held_dpad_repeats_through_the_event_system() {
    let (s, [a, b, c]) = row();
    let mut rig = Rig::new(s, Handlers::default());
    rig.events.set_selected(Some(a));
    rig.input.press("PADRIGHT");
    rig.tick();
    assert_eq!(rig.events.selected(), Some(b), "t = 0: the press moves");
    for _ in 0..29 {
        rig.tick();
    }
    assert_eq!(rig.events.selected(), Some(b), "under the delay: no repeat");
    rig.tick();
    assert_eq!(
        rig.events.selected(),
        Some(c),
        "at the 0.5 s delay: repeats"
    );
}

#[test]
fn the_stick_moves_once_by_its_dominant_axis_past_its_press_point() {
    let (s, [a, b, _]) = row();
    let mut rig = Rig::new(s, Handlers::default());
    rig.events.set_selected(Some(b));
    rig.input.set_axis("PADLEFTX", 0.3);
    rig.tick();
    assert_eq!(rig.events.selected(), Some(b), "a slight lean does nothing");
    rig.input.set_axis("PADLEFTX", -0.8);
    rig.input.set_axis("PADLEFTY", 0.6);
    rig.tick();
    assert_eq!(rig.events.selected(), Some(a), "diagonal: left dominates");
    rig.tick();
    assert_eq!(rig.events.selected(), Some(a), "still held: one Move only");
    rig.input.set_axis("PAD1LEFTX", 0.0);
    rig.input.set_axis("PADLEFTX", 0.0);
    rig.input.set_axis("PADLEFTY", 0.0);
    rig.tick();
    rig.input.set_axis("PAD1LEFTX", 1.0);
    rig.tick();
    assert_eq!(
        rig.events.selected(),
        Some(b),
        "re-centred, then pad 1 pushes"
    );
}

#[test]
fn south_submits_and_east_cancels_the_focus() {
    let (s, [a, ..]) = row();
    let h = Handlers::default().on(a, &[Submit, Cancel]);
    let mut rig = Rig::new(s, h);
    rig.events.set_selected(Some(a));
    rig.tick();
    rig.input.press("PADA");
    rig.input.press("PAD3B");
    assert_eq!(rig.hooks(), vec![(a, Submit), (a, Cancel)]);
}
