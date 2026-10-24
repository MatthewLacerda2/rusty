//! The playheads as the `animate` system moves them (#80, #316): [`advance`] scales
//! by the animator's and the node's speed, holds while stopped or paused, wraps a
//! looping clip byte-identically at its length, and resolves a crossfade while the
//! outgoing playhead runs on unwrapped.

use super::advance;
use super::test_rig::{mesh, slide, HIPS};
use crate::components::{AnimatorComponent, Playback};

fn idle() -> AnimatorComponent {
    AnimatorComponent {
        base: Playback {
            current_clip: "Idle".to_string(),
            ..Default::default()
        },
        speed: 1.0,
        is_playing: true,
        ..Default::default()
    }
}

/// Step `anim` by `dt` over a mesh whose `Idle` and `Walk` clips last `length`
/// seconds (0: a clip that never wraps).
fn step(anim: &mut AnimatorComponent, dt: f32, length: f32) {
    let clips = ["Idle", "Walk"].map(|n| slide(n, &[HIPS], 0.0, 1.0, length));
    advance(anim, Some(&mesh(clips.to_vec())), None, dt);
}

#[test]
fn crossfade_blends_then_resolves_to_target() {
    let mut anim = idle();
    anim.base.time = 2.0;
    anim.crossfade("Walk".to_string(), 0.4);
    assert!(anim.is_crossfading());
    // The outgoing clip is captured with its playhead frozen.
    assert_eq!(anim.base.previous_clip.as_deref(), Some("Idle"));
    assert_eq!(anim.base.previous_time, 2.0);
    assert_eq!(
        (anim.base.current_clip.as_str(), anim.base.time),
        ("Walk", 0.0)
    );
    step(&mut anim, 0.2, 0.0); // half-way through the 0.4 s fade
    assert!((anim.crossfade_weight() - 0.5).abs() < 1e-6);
    assert!(anim.is_crossfading());
    step(&mut anim, 0.2, 0.0); // reaches the end — crossfade resolves
    assert!(!anim.is_crossfading());
    assert_eq!(anim.crossfade_weight(), 1.0);
    assert_eq!(anim.base.current_clip, "Walk");
    assert!(anim.base.previous_clip.is_none());
}

#[test]
fn speed_scales_the_playhead_and_stop_or_pause_holds_it() {
    let mut anim = idle();
    anim.speed = 2.0;
    step(&mut anim, 0.5, 0.0);
    assert!((anim.base.time - 1.0).abs() < 1e-6);
    anim.freeze = true; // Animator.Pause
    step(&mut anim, 0.5, 0.0);
    assert_eq!(anim.base.time, 1.0, "pause holds the playhead");
    assert!(anim.is_playing, "pause is a hold, not a stop");
    anim.freeze = false; // Animator.Resume
    step(&mut anim, 0.25, 0.0);
    assert_eq!(
        anim.base.time, 1.5,
        "resume continues from the held playhead"
    );
    anim.is_playing = false;
    step(&mut anim, 0.5, 0.0);
    assert_eq!(anim.base.time, 1.5, "stopped holds the playhead");
}

#[test]
fn component_and_node_speed_stack() {
    let mut anim = AnimatorComponent {
        base: Playback {
            node_speed: 0.5,
            ..Default::default()
        },
        is_playing: true,
        speed: 2.0,
        ..Default::default()
    };
    advance(&mut anim, None, None, 1.0);
    assert_eq!(
        anim.base.time, 1.0,
        "dt * speed * node_speed = 1.0 * 2.0 * 0.5"
    );
}

#[test]
fn looping_clip_wraps_byte_identically_across_periods() {
    let mut anim = idle();
    anim.base.loop_clip = true;
    // Two full periods of a 1 s clip at dt = 0.25: the playhead sequence must be
    // bit-for-bit the same each period (deterministic replay).
    let period = |anim: &mut AnimatorComponent| -> Vec<u32> {
        (0..4)
            .map(|_| {
                step(anim, 0.25, 1.0);
                anim.base.time.to_bits()
            })
            .collect()
    };
    let first = period(&mut anim);
    assert_eq!(
        first,
        period(&mut anim),
        "looping must replay identical playheads"
    );
    assert_eq!(
        anim.base.time, 0.0,
        "the period boundary wraps to exactly 0"
    );
}

#[test]
fn non_looping_clip_clamps_at_the_end_and_holds() {
    let mut anim = idle();
    step(&mut anim, 1.5, 1.0);
    assert_eq!(
        anim.base.time, 1.0,
        "without loop_clip the playhead clamps to the clip's length (#1362)"
    );
}

#[test]
fn loop_wrap_keeps_the_outgoing_crossfade_playhead_independent() {
    let mut anim = idle();
    anim.base.time = 0.9;
    anim.base.loop_clip = true;
    anim.crossfade("Walk".to_string(), 1.0);
    // The looping incoming clip (0.3 s long) wraps; the outgoing playhead just
    // keeps advancing, so the fading-out pose cannot glitch.
    step(&mut anim, 0.5, 0.3);
    assert!(
        (anim.base.time - 0.2).abs() < 1e-6,
        "0.5 wraps to 0.2 in a 0.3 s clip"
    );
    assert!(
        (anim.base.previous_time - 1.4).abs() < 1e-6,
        "outgoing playhead never wraps"
    );
    assert!(anim.is_crossfading());
}
