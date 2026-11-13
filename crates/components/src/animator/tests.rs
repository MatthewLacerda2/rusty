//! Unit tests for the animator's minimal state machine (#80): play resets and
//! starting a crossfade. How the playheads advance is `app::animation`'s
//! (`playhead_tests.rs`), through the system's own `advance`.

use super::{AnimatorComponent, Playback};

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

#[test]
fn play_resets_playhead_and_clears_crossfade() {
    let mut anim = idle();
    anim.base.time = 3.0;
    anim.crossfade("Walk".to_string(), 0.5); // start a crossfade...
    anim.play("Run".to_string()); // ...then hard-cut over it.
    assert_eq!(anim.base.current_clip, "Run");
    assert_eq!(anim.base.time, 0.0);
    assert!(anim.is_playing);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.crossfade_weight(), 1.0);
}

#[test]
fn crossfade_to_same_clip_is_a_plain_play() {
    let mut anim = idle();
    anim.base.time = 1.5;
    anim.crossfade("Idle".to_string(), 0.5);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.base.time, 0.0);
}

#[test]
fn zero_duration_crossfade_is_a_plain_play() {
    let mut anim = idle();
    anim.crossfade("Walk".to_string(), 0.0);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.base.current_clip, "Walk");
}

#[test]
fn graph_reference_defaults_to_none_and_round_trips_by_path() {
    // The #315 path reference: absent it stays out of the JSON; set, it round-trips — the path, never the graph contents.
    let anim = AnimatorComponent::default();
    assert_eq!(anim.graph, None);
    assert!(!serde_json::to_string(&anim).unwrap().contains("graph"));

    let mut anim = idle();
    anim.graph = Some("assets/anims/guard.animgraph".to_string());
    let json = serde_json::to_string(&anim).unwrap();
    let back: AnimatorComponent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.graph.as_deref(), Some("assets/anims/guard.animgraph"));
}
