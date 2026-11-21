//! What a velocity does besides move the fader.
//!
//! Two of these matter more than the others. One shows the brightness moving
//! when a recipe asks for it — the point of the routing. The other shows
//! *nothing* moving when it does not: a bake is addressed by the hash of its
//! recipe, so a recipe that never mentions velocity routing has to keep
//! producing the samples it always did, or `generated/` starts serving files
//! no recipe describes.

use crate::common::{brightness, minimal, opts, peak, render, saw_patch};
use scorsese_zimmer::NoteOpts;
use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Patch, Slope, Source};

/// Half a second of `patch`, struck at `velocity`.
fn struck(patch: &Patch, velocity: f32) -> Vec<f32> {
    render(
        patch,
        45.0,
        &NoteOpts {
            velocity,
            ..opts(0.5)
        },
    )
}

/// A lowpass with no envelope movement of its own, so velocity is the only
/// thing left that can move the cutoff.
fn lowpass(cutoff: f32, vel_octaves: f32) -> Filter {
    Filter {
        kind: FilterKind::Lowpass,
        slope: Slope::Db12,
        cutoff,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves,
        key_track: 0.0,
        adsr: Adsr::default(),
    }
}

/// Two-operator FM whose modulator holds up for the whole note, so brightness
/// is `index` and nothing else.
fn fm(index: f32, vel_index: f32) -> Patch {
    minimal(Source::Fm2 {
        ratio: 2.0,
        index,
        vel_index,
        mod_decay: 10.0,
    })
}

#[test]
fn vel_octaves_makes_a_hard_strike_brighter_than_a_soft_one() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(400.0, 3.5));
    let soft = brightness(&struck(&patch, 0.2));
    let hard = brightness(&struck(&patch, 1.0));
    assert!(
        hard > soft * 2.0,
        "a hard strike opens the filter: {soft} → {hard}"
    );
}

#[test]
fn vel_index_makes_a_hard_strike_brighter_on_fm() {
    let patch = fm(1.0, 8.0);
    let soft = brightness(&struck(&patch, 0.1));
    let hard = brightness(&struck(&patch, 1.0));
    assert!(
        hard > soft * 2.0,
        "a hard strike adds sidebands: {soft} → {hard}"
    );
}

/// The one that protects every bake made before these fields existed: with
/// neither field set, a quarter-velocity note is the full-velocity note times a
/// quarter, sample for sample — velocity is a fader and touches nothing else.
#[test]
fn velocity_alone_is_still_only_a_fader() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(1200.0, 0.0));
    let hard = struck(&patch, 1.0);
    let soft = struck(&patch, 0.25);
    assert!(peak(&hard) > 0.1, "there is a signal to compare at all");
    for (loud, quiet) in hard.iter().zip(&soft) {
        assert!(
            (loud * 0.25 - quiet).abs() < 1e-6,
            "the soft note is the loud one scaled, not a different sound"
        );
    }
}

/// A note struck at one level can still be played brighter or duller: that is
/// the whole of `timbre`, and it reaches a patch through the same two routings
/// a velocity does.
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

/// And where a patch names no brightness routing there is nothing for it to
/// move: an instrument decides what "brighter" means, and one that never said
/// is not one to guess for.
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

/// Negative is legal, and it is a real instrument rather than a mistake to
/// tolerate: a sound that closes down as it is played harder.
#[test]
fn a_negative_vel_octaves_darkens_instead_of_brightening() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(6000.0, -5.0));
    let soft = brightness(&struck(&patch, 0.1));
    let hard = brightness(&struck(&patch, 1.0));
    assert!(
        hard < soft * 0.5,
        "harder is darker when the routing says so: {soft} → {hard}"
    );
}

/// Nothing a recipe writes in these fields can produce an unstable filter: the
/// cutoff is clamped into the SVF's stable band per sample, exactly as it
/// already was for a `cutoff` swept off the end by `env_octaves`.
#[test]
fn an_extreme_vel_octaves_cannot_destabilise_the_filter() {
    for vel_octaves in [-1e9, 1e9] {
        let mut patch = saw_patch();
        patch.filter = Some(Filter {
            resonance: 0.99,
            ..lowpass(1000.0, vel_octaves)
        });
        let buf = struck(&patch, 1.0);
        assert!(
            buf.iter().all(|sample| sample.is_finite()),
            "vel_octaves {vel_octaves} produced non-finite samples"
        );
    }
}

/// A negative FM index only mirrors the modulator, so it sounds exactly as
/// bright as its positive twin. The sum is floored at zero so a darkening
/// routing bottoms out at a bare sine rather than turning around.
#[test]
fn a_negative_vel_index_bottoms_out_rather_than_brightening_again() {
    let dulled = brightness(&struck(&fm(2.0, -1.0), 1.0));
    let far_past = brightness(&struck(&fm(2.0, -8.0), 1.0));
    assert!(
        far_past < dulled,
        "past zero it is a plain carrier, not a bright one: {dulled} → {far_past}"
    );
}
