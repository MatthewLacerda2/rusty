//! Unit tests for the animator's minimal state machine (#80): play resets, the
//! crossfade lifecycle, and playhead advancement.

use super::AnimatorComponent;

fn idle() -> AnimatorComponent {
    AnimatorComponent {
        current_clip: "Idle".to_string(),
        speed: 1.0,
        is_playing: true,
        ..Default::default()
    }
}

#[test]
fn play_resets_playhead_and_clears_crossfade() {
    let mut anim = idle();
    anim.time = 3.0;
    anim.crossfade("Walk".to_string(), 0.5); // start a crossfade...
    anim.play("Run".to_string()); // ...then hard-cut over it.
    assert_eq!(anim.current_clip, "Run");
    assert_eq!(anim.time, 0.0);
    assert!(anim.is_playing);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.crossfade_weight(), 1.0);
}

#[test]
fn crossfade_blends_then_resolves_to_target() {
    let mut anim = idle();
    anim.time = 2.0;
    anim.crossfade("Walk".to_string(), 0.4);
    // The outgoing clip is captured with its playhead frozen.
    assert_eq!(anim.previous_clip.as_deref(), Some("Idle"));
    assert_eq!(anim.previous_time, 2.0);
    assert_eq!(anim.current_clip, "Walk");
    assert_eq!(anim.time, 0.0);
    assert!(anim.is_crossfading());

    anim.advance(0.2, 0.0); // half-way through the 0.4 s fade
    assert!((anim.crossfade_weight() - 0.5).abs() < 1e-6);
    assert!(anim.is_crossfading());

    anim.advance(0.2, 0.0); // reaches the end — crossfade resolves
    assert!(!anim.is_crossfading());
    assert_eq!(anim.crossfade_weight(), 1.0);
    assert_eq!(anim.current_clip, "Walk");
    assert!(anim.previous_clip.is_none());
}

#[test]
fn crossfade_to_same_clip_is_a_plain_play() {
    let mut anim = idle();
    anim.time = 1.5;
    anim.crossfade("Idle".to_string(), 0.5);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.time, 0.0);
}

#[test]
fn zero_duration_crossfade_is_a_plain_play() {
    let mut anim = idle();
    anim.crossfade("Walk".to_string(), 0.0);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.current_clip, "Walk");
}

#[test]
fn advance_scales_playhead_by_speed_and_respects_freeze() {
    let mut anim = idle();
    anim.speed = 2.0;
    anim.advance(0.5, 0.0);
    assert!((anim.time - 1.0).abs() < 1e-6);

    anim.freeze = true;
    anim.advance(0.5, 0.0);
    assert!(
        (anim.time - 1.0).abs() < 1e-6,
        "freeze should halt the playhead"
    );

    anim.freeze = false;
    anim.is_playing = false;
    anim.advance(0.5, 0.0);
    assert!(
        (anim.time - 1.0).abs() < 1e-6,
        "stopped should halt the playhead"
    );
}

#[test]
fn looping_clip_wraps_byte_identically_across_periods() {
    let mut anim = idle();
    anim.loop_clip = true;
    // Two full periods of a 1 s clip at dt = 0.25: the playhead sequence must be
    // bit-for-bit the same each period (deterministic replay).
    let period = |anim: &mut AnimatorComponent| -> Vec<u32> {
        (0..4)
            .map(|_| {
                anim.advance(0.25, 1.0);
                anim.time.to_bits()
            })
            .collect()
    };
    let first = period(&mut anim);
    let second = period(&mut anim);
    assert_eq!(first, second, "looping must replay identical playheads");
    assert_eq!(anim.time, 0.0, "the period boundary wraps to exactly 0");
}

#[test]
fn non_looping_clip_runs_past_the_end_and_holds() {
    let mut anim = idle();
    anim.advance(1.5, 1.0);
    assert!(
        (anim.time - 1.5).abs() < 1e-6,
        "without loop_clip the playhead is never wrapped (sampler holds last frame)"
    );
}

#[test]
fn pause_freezes_the_playhead_and_resume_continues() {
    let mut anim = idle();
    anim.advance(0.25, 0.0);
    anim.freeze = true; // Animator.Pause
    anim.advance(0.25, 0.0);
    assert_eq!(anim.time, 0.25, "pause holds the playhead");
    assert!(anim.is_playing, "pause is a hold, not a stop");
    anim.freeze = false; // Animator.Resume
    anim.advance(0.25, 0.0);
    assert_eq!(anim.time, 0.5, "resume continues from the held playhead");
}

#[test]
fn loop_wrap_keeps_the_outgoing_crossfade_playhead_independent() {
    let mut anim = idle();
    anim.time = 0.9;
    anim.loop_clip = true;
    anim.crossfade("Walk".to_string(), 1.0);
    // The looping incoming clip (0.3 s long) wraps; the outgoing playhead just
    // keeps advancing, so the fading-out pose cannot glitch.
    anim.advance(0.5, 0.3);
    assert!(
        (anim.time - 0.2).abs() < 1e-6,
        "0.5 wraps to 0.2 in a 0.3 s clip"
    );
    assert!(
        (anim.previous_time - 1.4).abs() < 1e-6,
        "outgoing playhead never wraps"
    );
    assert!(anim.is_crossfading());
}
