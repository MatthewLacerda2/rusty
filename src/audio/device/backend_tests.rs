//! The kira backend, heard through the capture backend (#465): the pan law, master
//! gain, rate, pause, one-shot reaping and groups, as the device renders them.

use super::*;
use crate::audio::device::capture::{Capture, RATE};
use crate::audio::device::decode::CachedClip;

/// A backend over the capture device with Master and one child group (`1`), and
/// a mono clip `"clip"` holding `samples` at the device rate.
pub(crate) fn rig(samples: &[f32]) -> KiraBackend<Capture> {
    let mut b = KiraBackend::<Capture>::with_backend(()).expect("capture starts");
    b.add_group(GroupId::MASTER, None);
    b.add_group(GroupId(1), Some(GroupId::MASTER));
    b.cache()
        .insert("clip", CachedClip::new(1, RATE, samples.to_vec()));
    b
}

/// Play `clip` once, flat, through Master.
pub(crate) fn flat(clip: &str) -> PlayParams {
    PlayParams {
        clip: clip.to_string(),
        looping: false,
        mix: VoiceMix::FLAT,
        group: GroupId::MASTER,
    }
}

const TONE: [f32; 4] = [0.5; 4];

#[test]
fn a_centred_voice_plays_at_its_gain_on_both_channels() {
    let mut b = rig(&TONE);
    let params = PlayParams {
        mix: VoiceMix::FLAT.with_master(0.5),
        ..flat("clip")
    };
    assert!(b.play(VoiceId(1), &params));
    assert_eq!(b.device().render(2), vec![(0.25, 0.25); 2]);
}

#[test]
fn hard_left_silences_the_right_channel_and_retunes_in_place() {
    let mut b = rig(&[0.5; 8]);
    let left = VoiceMix {
        pan: -1.0,
        ..VoiceMix::FLAT
    };
    assert!(b.play(
        VoiceId(1),
        &PlayParams {
            mix: left,
            ..flat("clip")
        }
    ));
    assert_eq!(b.device().render(2), vec![(0.5, 0.0); 2]);
    b.set_mix(VoiceId(1), &VoiceMix { pan: 1.0, ..left });
    assert_eq!(
        b.device().render(2),
        vec![(0.0, 0.5); 2],
        "same voice, panned"
    );
}

#[test]
fn a_one_shot_reaps_once_it_runs_out_and_a_loop_does_not() {
    let mut b = rig(&TONE);
    assert!(b.play(VoiceId(1), &flat("clip")));
    let looped = PlayParams {
        looping: true,
        ..flat("clip")
    };
    assert!(b.play(VoiceId(2), &looped));
    b.device().render(16);
    assert!(!b.is_live(VoiceId(1)));
    assert!(b.is_live(VoiceId(2)));
    assert_eq!(
        b.device().render(1),
        vec![(0.5, 0.5)],
        "the loop still sounds"
    );
    b.stop(VoiceId(2));
    assert!(!b.is_live(VoiceId(2)));
    assert!(
        !b.play(VoiceId(3), &flat("missing.ogg")),
        "undecodable clip"
    );
}

#[test]
fn rate_resamples_and_pause_holds_the_playhead() {
    let mut b = rig(&[0.0, 0.5, 1.0, 1.0]);
    let half = VoiceMix {
        speed: 0.5,
        ..VoiceMix::FLAT
    };
    assert!(b.play(
        VoiceId(1),
        &PlayParams {
            mix: half,
            ..flat("clip")
        }
    ));
    let out: Vec<f32> = b.device().render(3).iter().map(|f| f.0).collect();
    assert_eq!(out, vec![0.0, 0.25, 0.5], "half speed interpolates");
    b.set_mix(
        VoiceId(1),
        &VoiceMix {
            paused: true,
            ..half
        },
    );
    assert_eq!(b.device().render(4), vec![(0.0, 0.0); 4]);
    b.set_mix(VoiceId(1), &VoiceMix::FLAT);
    assert_eq!(b.device().render(1)[0].0, 0.75, "resumes where it stopped");
}
