//! What each noise colour actually is, measured off the rendered signal.
//!
//! A colour is a **slope**, so every claim here is a fitted line across octaves
//! rather than a comparison between two bands: a lowpass over white would pass
//! any two-band check and is precisely the thing these colours exist not to be.
//! [`crate::common::spectrum`] is the measurement.

use crate::common::spectrum::{octave_db, slope_db_per_octave};
use crate::common::{channel, minimal, opts, peak, render_stereo};
use scorsese_zimmer::Patch;
use scorsese_zimmer::patch::{Adsr, NoiseColor, Source};

/// Two seconds of a colour, held flat by an envelope that does nothing, so
/// what is measured is the source and not an amp shape.
fn held(color: NoiseColor) -> Vec<f32> {
    let patch = Patch {
        amp: Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
            curve: 0.0,
        },
        ..minimal(Source::Noise { color })
    };
    channel(&render_stereo(&patch, 60.0, &opts(2.0)), 0)
}

/// The lowest octave centre the slopes are fitted from, and how many octaves.
/// From 125 Hz up is the range a recipe is writing for, and it is clear of
/// both the brown integrator's 35 Hz corner and the top of the analysis band.
const LOWEST_HZ: f32 = 125.0;
const OCTAVES: usize = 7;

#[test]
fn white_is_flat_across_the_whole_range() {
    let slope = slope_db_per_octave(&held(NoiseColor::White), LOWEST_HZ, OCTAVES);
    assert!(
        slope.abs() < 0.5,
        "white is sloping at {slope:+.2} dB/octave"
    );
}

/// Pink is −3 dB per octave — equal energy per octave — and it is that slope
/// all the way rather than a corner with flat either side.
#[test]
fn pink_falls_three_decibels_an_octave() {
    let pink = held(NoiseColor::Pink);
    let slope = slope_db_per_octave(&pink, LOWEST_HZ, OCTAVES);
    assert!(
        (slope + 3.0).abs() < 0.7,
        "pink is sloping at {slope:+.2} dB/octave"
    );
    // Every octave is genuinely lower than the one below it. A filter that
    // reached the right average slope by being flat and then falling off a
    // cliff would fail here and pass the fit above.
    let bands = octave_db(&pink, LOWEST_HZ, OCTAVES);
    for pair in bands.windows(2) {
        let step = pair[1] - pair[0];
        assert!(
            (-6.0..-1.0).contains(&step),
            "an octave stepped {step:+.2} dB"
        );
    }
}

/// Brown is −6 dB per octave: twice pink's fall, which is what puts its energy
/// in the rumble instead of the hiss.
#[test]
fn brown_falls_six_decibels_an_octave() {
    let brown = held(NoiseColor::Brown);
    let slope = slope_db_per_octave(&brown, LOWEST_HZ, OCTAVES);
    assert!(
        (slope + 6.0).abs() < 0.8,
        "brown is sloping at {slope:+.2} dB/octave"
    );
    let bands = octave_db(&brown, LOWEST_HZ, OCTAVES);
    for pair in bands.windows(2) {
        let step = pair[1] - pair[0];
        assert!(
            (-9.0..-3.5).contains(&step),
            "an octave stepped {step:+.2} dB"
        );
    }
}

/// The three are three different slopes and not one signal at three volumes —
/// the claim a level-matched set of colours could otherwise quietly fail.
#[test]
fn each_colour_is_a_steeper_fall_than_the_last() {
    let of = |color| slope_db_per_octave(&held(color), LOWEST_HZ, OCTAVES);
    let (white, pink, brown) = (
        of(NoiseColor::White),
        of(NoiseColor::Pink),
        of(NoiseColor::Brown),
    );
    assert!(white > pink + 2.0, "white {white:+.2} vs pink {pink:+.2}");
    assert!(pink > brown + 2.0, "pink {pink:+.2} vs brown {brown:+.2}");
}

/// Changing colour is not changing the fader. Measured on the rendered note
/// rather than inside the generator, so the whole path is included.
#[test]
fn switching_colour_is_not_a_volume_change() {
    let level = |color| {
        let buf = held(color);
        (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
    };
    let white = level(NoiseColor::White);
    for color in [NoiseColor::Pink, NoiseColor::Brown] {
        let db = 20.0 * (level(color) / white).log10();
        assert!(db.abs() < 0.6, "{color:?} is {db:+.2} dB from white");
    }
}

/// White is the default and the default is the hiss the source has always
/// been: the same samples, and a document that never mentions colour.
#[test]
fn a_recipe_that_says_nothing_gets_the_white_it_always_did() {
    let stated = minimal(Source::Noise {
        color: NoiseColor::White,
    });
    let json = stated.to_json().expect("serialise");
    assert!(!json.contains("color"), "white was written into {json}");
    let parsed: Patch = Patch::from_json(
        r#"{"source":{"kind":"noise"},"amp":{"a":0.005,"d":0.0,"s":1.0,"r":0.05}}"#,
    )
    .expect("a recipe written before colour existed still parses");
    assert_eq!(parsed.source, stated.source);
    assert_eq!(
        render_stereo(&parsed, 60.0, &opts(0.2)),
        render_stereo(&stated, 60.0, &opts(0.2))
    );
}

/// A colour is written down as the word the recipe spells it with, and comes
/// back as itself.
#[test]
fn a_stated_colour_round_trips_as_the_word_it_was_written_as() {
    for (color, word) in [(NoiseColor::Pink, "pink"), (NoiseColor::Brown, "brown")] {
        let patch = minimal(Source::Noise { color });
        let json = patch.to_json().expect("serialise");
        assert!(json.contains(word), "{word} is missing from {json}");
        assert_eq!(Patch::from_json(&json).expect("deserialise"), patch);
    }
}

/// A coloured source is a Gaussian-ish signal at white's RMS, so it peaks
/// higher than the uniform draw does — which is what it is, and is bounded.
#[test]
fn a_coloured_source_peaks_where_its_crest_factor_puts_it() {
    assert!(peak(&held(NoiseColor::White)) <= 1.0 + 1e-3);
    for color in [NoiseColor::Pink, NoiseColor::Brown] {
        let peak = peak(&held(color));
        assert!((1.2..3.0).contains(&peak), "{color:?} peaked at {peak}");
    }
}
