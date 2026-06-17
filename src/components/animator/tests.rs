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

    anim.advance(0.2); // half-way through the 0.4 s fade
    assert!((anim.crossfade_weight() - 0.5).abs() < 1e-6);
    assert!(anim.is_crossfading());

    anim.advance(0.2); // reaches the end — crossfade resolves
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
    anim.advance(0.5);
    assert!((anim.time - 1.0).abs() < 1e-6);

    anim.freeze = true;
    anim.advance(0.5);
    assert!(
        (anim.time - 1.0).abs() < 1e-6,
        "freeze should halt the playhead"
    );

    anim.freeze = false;
    anim.is_playing = false;
    anim.advance(0.5);
    assert!(
        (anim.time - 1.0).abs() < 1e-6,
        "stopped should halt the playhead"
    );
}
