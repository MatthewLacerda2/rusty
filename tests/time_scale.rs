//! `Time.timeScale` global slow-mo / pause (issue #26).
//! Drives a known motion (the player walking forward) through the headless
//! harness and asserts `time_scale` scales the integrated displacement:
//! `0.0` freezes it, `0.5` halves it. Gated on `dev` (needs the harness).
#![cfg(feature = "dev")]

use rusty::dev::harness::Harness;

/// Hold "W", run `frames` ticks at `scale`, return the player's forward travel.
fn travel_at_scale(scale: f32, frames: u32) -> f32 {
    let h = Harness::new(std::env::temp_dir().join("rusty_time_scale"), "");

    let (player_id, start) = {
        let world = h.world.borrow();
        world.time().borrow_mut().set_time_scale(scale);
        world.input().borrow_mut().set_key_state("W", true);
        let scene = world.scene().borrow();
        let id = scene.find_entity_by_name("Player").expect("Player exists");
        let pos = scene.get_entity(id).unwrap().transform.position;
        (id, pos)
    };

    h.step(frames);

    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let end = scene.get_entity(player_id).unwrap().transform.position;
    (end - start).length()
}

#[test]
fn time_scale_zero_freezes_motion() {
    let frozen = travel_at_scale(0.0, 60);
    assert!(
        frozen < 1e-4,
        "time_scale = 0 must freeze the sim, moved {frozen}"
    );
}

#[test]
fn time_scale_half_halves_motion() {
    let full = travel_at_scale(1.0, 60);
    let half = travel_at_scale(0.5, 60);
    assert!(full > 0.1, "baseline motion should be non-trivial: {full}");
    assert!(
        (half - full * 0.5).abs() < 1e-3,
        "time_scale = 0.5 should halve motion: full={full} half={half}"
    );
}
