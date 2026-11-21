//! Timbre: touch moves the tone through the patch's velocity routes, without the fader.

use crate::common::{brightness, opts, peak, render, saw_patch};
use rusty_zimmer::NoteOpts;

use super::{fm, lowpass};

#[test]
fn timbre_brightens_a_note_without_striking_it_harder() {
    let mut filtered = saw_patch();
    filtered.filter = Some(lowpass(400.0, 3.5));
    for patch in [filtered, fm(1.0, 8.0)] {
        let played = |timbre: f32| {
            render(
                &patch,
                45.0,
                &NoteOpts {
                    velocity: 0.5,
                    timbre,
                    ..opts(0.5)
                },
            )
        };
        let duller = brightness(&played(-0.3));
        let brighter = brightness(&played(0.3));
        assert!(
            brighter > duller * 1.5,
            "touch did not move the tone: {duller} -> {brighter}"
        );
    }
}

#[test]
fn timbre_is_silent_on_a_patch_that_routes_no_brightness() {
    let patch = saw_patch();
    let played = |timbre: f32| {
        render(
            &patch,
            45.0,
            &NoteOpts {
                velocity: 0.5,
                timbre,
                ..opts(0.5)
            },
        )
    };
    let plain = played(0.0);
    assert!(peak(&plain) > 0.1, "there is a signal to compare at all");
    assert_eq!(
        plain,
        played(0.4),
        "brighter moved a note that has no route"
    );
    assert_eq!(plain, played(-0.4), "and neither may duller");
}
