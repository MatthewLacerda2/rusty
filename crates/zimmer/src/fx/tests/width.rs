//! Which effects widen a centred signal and which leave it centred.

use super::*;

#[test]
fn a_room_goes_off_centre() {
    let mut room = impulse(22_050);
    apply_chain(
        &mut room,
        &[Fx::Reverb {
            size: 0.7,
            damp: 0.5,
            mix: 1.0,
            predelay: 0.0,
        }],
        44_100.0,
    );
    assert_ne!(room.l, room.r, "a room has two sides");
}

#[test]
fn an_ensemble_goes_off_centre() {
    let mut section = impulse(22_050);
    apply_chain(
        &mut section,
        &[Fx::Chorus {
            rate: 0.8,
            depth: 0.7,
            voices: 4,
            mix: 1.0,
        }],
        44_100.0,
    );
    assert_ne!(section.l, section.r, "the copies are placed apart");
}

#[test]
fn a_plain_delay_and_a_waveshaper_stay_centred() {
    let mut narrow = impulse(22_050);
    apply_chain(
        &mut narrow,
        &[
            Fx::Delay {
                time: 0.05,
                beats: None,
                feedback: 0.4,
                mix: 0.5,
                ping_pong: false,
            },
            Fx::Saturate {
                drive: 4.0,
                mix: 1.0,
            },
        ],
        44_100.0,
    );
    assert_eq!(narrow.l, narrow.r, "neither of these is a width effect");
}

#[test]
fn a_ping_pong_goes_off_centre() {
    let mut walked = impulse(22_050);
    apply_chain(
        &mut walked,
        &[Fx::Delay {
            time: 0.05,
            beats: None,
            feedback: 0.4,
            mix: 0.5,
            ping_pong: true,
        }],
        44_100.0,
    );
    assert_ne!(walked.l, walked.r, "the repeats crossed over");
}
