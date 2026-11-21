//! ensemble — several detuned, delayed copies of one signal, spread wide.
//!
//! The standard answer to *this is one synthesiser playing one note*, and the
//! reason is mechanical rather than tasteful. Five violinists are not one
//! violinist five times louder: they are five copies that disagree slightly
//! about pitch and about when the note started, and **the disagreement is the
//! sound**. Nothing else in this crate can make one source sound like more
//! than one thing — a filter, an envelope and a waveshaper each change what
//! the single thing sounds like, and a reverb puts the single thing in a room.
//!
//! Each voice is a delay line of about [`BASE_SECONDS`], read at a position a
//! slow sine walks back and forth by up to [`SWEEP_SECONDS`]. A moving read
//! position is a resampling, so a voice is *pitch-shifted* while the sweep
//! moves — sharp on the way in, flat on the way out — which is the detune. The
//! delays themselves are short enough that the copies fuse into one thickened
//! event rather than being heard as echoes.
//!
//! ## The two halves, and where each lives
//!
//! [`line`] is the delay and the reader, and it carries the argument for
//! **linear interpolation** — the detail that separates a chorus from a broken
//! one, because a modulated delay read at whole sample positions is a
//! staircase of discontinuities rather than a glide in pitch.
//!
//! [`voices`] is who the copies are: how many, where each sweep starts, how
//! fast it runs and where it sits. It carries the determinism argument — every
//! number in it comes from [`crate::hash`] under a fixed seed, so the ensemble
//! is a pure function of the effect's own settings and never of a counter
//! carried between calls or of where in a buffer processing began. A
//! free-running LFO would break that the moment the same chain ran on a bus
//! instead of on a note.
//!
//! ## Where the width comes from
//!
//! The voices read a **mono sum** of the input and are then panned across the
//! field, the way [`super::reverb`] sends mono into a stereo room. So a
//! centred source comes back wide, which is the point: the modulated copies
//! sitting outside the dry signal is half of what a chorus is, and the half a
//! mono implementation cannot have.
//!
//! `voices` is neither a fader nor a width control, and both of those read the
//! other way round — [`voices`] argues each where the placement is done.
//!
//! ## A flanger is a separate variant, and is not this one
//!
//! The structures look alike — a flanger is also a modulated delay line — and
//! this effect deliberately **cannot** be turned into one. A flanger's delay
//! is an order of magnitude shorter, down where the comb notches land inside
//! the range the ear reads as tone colour, and it feeds its output back into
//! its own input, which is where the resonant jet-sweep comes from. Without
//! the feedback path it is not a flanger; with one, this variant would carry a
//! field that means nothing for the ensemble it is named after, and a `voices`
//! count that means nothing for the flanger.
//!
//! So if a recipe ever needs a flanger it gets its own `Fx` variant, with its
//! own delay range and its own feedback. That is the layer's standing rule —
//! an effect arrives when a real sound cannot be made without it — and nothing
//! has needed one yet.

pub(crate) mod line;
pub(crate) mod voices;

use std::f32::consts::TAU;

use crate::stereo::Stereo;
use line::read;
use voices::{MAX_VOICES, MIN_VOICES, ensemble};

/// Where a voice's delay sits before the sweep moves it, in seconds.
///
/// The classic chorus centre. Much under 10 ms and the comb notches climb into
/// the range heard as tone colour rather than thickness — that is the flanger
/// the module doc refuses; much over 30 ms and the copies stop fusing into one
/// event and start being heard as a slapback.
const BASE_SECONDS: f32 = 0.015;

/// How far either side of [`BASE_SECONDS`] a voice sweeps at `depth` 1, in
/// seconds. Ten milliseconds over a cycle is a detune of a few cents at any
/// usable rate — audible as richness, not as being out of tune.
const SWEEP_SECONDS: f32 = 0.010;

/// The fastest the sweep may run, in Hz. Past this a chorus is a ring
/// modulator: the sidebands the modulation puts either side of every partial
/// stop reading as a detune and start being inharmonic tones of their own.
const MAX_RATE: f32 = 10.0;

/// Blend a modulated ensemble of `buf` into `buf`, in place.
///
/// `lfo_rate` is the sweep in Hz, `depth` how far it moves in `0..=1`,
/// `voices` how many copies (clamped to [`MIN_VOICES`]..=[`MAX_VOICES`]),
/// `mix` the wet/dry blend, and `rate` the sample rate.
pub(crate) fn apply(
    buf: &mut Stereo,
    lfo_rate: f32,
    depth: f32,
    voices: usize,
    mix: f32,
    rate: f32,
) {
    // Only the sample rate is guarded, and it is guarded because a rate of
    // zero gives a delay line too short to hold the taps that read it. A
    // `mix` of zero and an empty buffer were both early returns here once,
    // and both are gone: the blend at the bottom already hands back the dry
    // sample exactly at zero mix, and a buffer of no frames already runs the
    // loop no times. Neither clause could change an output, which makes them
    // branches to delete rather than branches to test.
    if !rate.is_finite() || rate <= 0.0 {
        return;
    }
    let mix = clamp_unit(mix);
    let depth = clamp_unit(depth);
    let lfo_rate = if lfo_rate.is_finite() {
        lfo_rate.clamp(0.0, MAX_RATE)
    } else {
        0.0
    };
    let mut ensemble = ensemble(voices.clamp(MIN_VOICES, MAX_VOICES), lfo_rate, rate);

    let len = line_len(depth, rate);
    let mut line = vec![0.0f32; len];
    let centre = BASE_SECONDS * rate;
    let sweep = depth * SWEEP_SECONDS * rate;

    for i in 0..buf.frames() {
        let (dry_l, dry_r) = (buf.l[i], buf.r[i]);
        line[i % len] = (dry_l + dry_r) * 0.5;
        let (mut wet_l, mut wet_r) = (0.0, 0.0);
        for voice in &mut ensemble {
            let tap = read(&line, i, centre + sweep * voice.phase.sin());
            wet_l += tap * voice.gains.0;
            wet_r += tap * voice.gains.1;
            voice.phase = advance(voice.phase, voice.step);
        }
        buf.l[i] = dry_l * (1.0 - mix) + wet_l * mix;
        buf.r[i] = dry_r * (1.0 - mix) + wet_r * mix;
    }
}

/// How many samples of delay line the deepest read needs behind it.
///
/// Two longer than that read, so the interpolator's second tap can never reach
/// the sample being written this iteration. It is a **lower bound and not a
/// size**: any longer line holds the same samples at the same distances behind
/// the write position, so nothing downstream can tell one from a longer one —
/// which is why `.cargo/mutants.toml` excludes the two mutations here that only
/// make it bigger, and why the one that makes it smaller is caught.
fn line_len(depth: f32, rate: f32) -> usize {
    (tail_seconds(depth) * rate).ceil() as usize + 2
}

/// One sample's worth of sweep, wrapped so the phase cannot grow without bound
/// and lose its precision over a long buffer.
///
/// Its own function so that the *direction* it advances in can be excluded by
/// name in `.cargo/mutants.toml`: a voice's starting phase is an arbitrary draw
/// from the hash, so sweeping backwards from it is sweeping forwards from a
/// different draw, and there is no behaviour between the two to assert.
fn advance(phase: f32, step: f32) -> f32 {
    (phase + step) % TAU
}

/// How long the ensemble rings on after the signal stops, in seconds: the
/// deepest a voice is ever read from, and nothing more.
///
/// A chorus has no feedback path, so a sample enters the line once and leaves
/// it once — unlike an echo or a room, where the tail is a decay that has to
/// be estimated. This one is exact, and it is a fortieth of a second.
pub(crate) fn tail_seconds(depth: f32) -> f32 {
    BASE_SECONDS + clamp_unit(depth) * SWEEP_SECONDS
}

/// A `0..=1` control, with anything that is not a number read as zero.
fn clamp_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    /// A second of a full-scale sine, dead centre.
    fn tone(freq: f32) -> Stereo {
        Stereo::centred(
            (0..RATE as usize)
                .map(|i| (TAU * freq * i as f32 / RATE).sin())
                .collect(),
        )
    }

    /// A single full-scale impulse in both channels, then silence — the
    /// fixture that turns "what did this do to the sound" into "where, and
    /// how much", which is what the arithmetic below can actually be read off.
    fn impulse(frames: usize) -> Stereo {
        let mut mono = vec![0.0; frames];
        mono[0] = 1.0;
        Stereo::centred(mono)
    }

    /// Every sample index carrying audible energy, over both channels.
    fn arrivals(buf: &Stereo) -> Vec<usize> {
        (0..buf.frames())
            .filter(|i| buf.l[*i].abs() + buf.r[*i].abs() > 1e-3)
            .collect()
    }

    /// How alike the two channels are, `1.0` being identical.
    fn correlation(buf: &Stereo) -> f32 {
        let dot: f32 = buf.l.iter().zip(&buf.r).map(|(l, r)| l * r).sum();
        let energy = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>().sqrt();
        dot / (energy(&buf.l) * energy(&buf.r)).max(f32::MIN_POSITIVE)
    }

    #[test]
    fn a_centred_source_comes_back_wide() {
        let dry = tone(440.0);
        assert!((correlation(&dry) - 1.0).abs() < 1e-4, "it went in centred");
        let mut wet = dry.clone();
        apply(&mut wet, 0.6, 0.8, 4, 1.0, RATE);
        assert_ne!(wet.l, wet.r, "the sides disagree");
        let widened = correlation(&wet);
        assert!(widened < 0.9, "the sides are still {widened} alike");
    }

    #[test]
    fn the_same_settings_render_the_same_samples() {
        // The determinism contract: no counter, no clock, no free-running
        // phase — two calls on two buffers are bit-identical.
        let mut once = tone(220.0);
        let mut again = tone(220.0);
        apply(&mut once, 1.3, 0.7, 3, 0.6, RATE);
        apply(&mut again, 1.3, 0.7, 3, 0.6, RATE);
        assert_eq!(once, again);
    }

    #[test]
    fn a_dry_mix_and_a_degenerate_setting_change_nothing() {
        let original = tone(330.0);
        let mut dry = original.clone();
        apply(&mut dry, 0.5, 0.5, 3, 0.0, RATE);
        assert_eq!(dry, original, "a dry mix is not an approximation");
        for (lfo_rate, depth, rate) in [
            (f32::NAN, 0.5, RATE),
            (0.5, f32::INFINITY, RATE),
            (0.5, 0.5, 0.0),
            (1e9, 2.0, RATE),
        ] {
            let mut buf = original.clone();
            apply(&mut buf, lfo_rate, depth, 3, 1.0, rate);
            assert!(buf.l.iter().chain(&buf.r).all(|s| s.is_finite()));
        }
    }

    /// Where a copy actually lands. At `depth` 0 every voice is frozen on the
    /// centre delay, which at this rate is 661.5 samples — deliberately not a
    /// whole number, so an impulse can only come back split evenly across two
    /// samples if the read is interpolated at all.
    #[test]
    fn a_copy_arrives_exactly_the_centre_delay_behind_the_dry_signal() {
        let mut buf = impulse(4096);
        apply(&mut buf, 0.0, 0.0, 4, 1.0, RATE);
        let at = BASE_SECONDS * RATE;
        assert!(
            (at.fract() - 0.5).abs() < 1e-3,
            "the fixture rests on a half"
        );
        let near = at.floor() as usize;
        // Both sides, and that is not belt and braces: the two channels are
        // summed and blended by separate lines of arithmetic, so a claim made
        // only about the left one leaves the right one unasserted.
        for side in [&buf.l, &buf.r] {
            assert!((side[near] - 0.5).abs() < 1e-4, "near tap {}", side[near]);
            assert!(
                (side[near + 1] - 0.5).abs() < 1e-4,
                "far tap {}",
                side[near + 1]
            );
            assert_eq!(side[near - 1], 0.0, "and silence either side of the pair");
            assert_eq!(side[near + 2], 0.0);
            assert_eq!(side[0], 0.0, "fully wet keeps no dry signal");
        }
    }

    /// What `depth` buys, held still where it can be read off: a rate of zero
    /// freezes each voice at its own phase, so instead of sweeping through its
    /// delays the ensemble sits at a fixed spread of them.
    #[test]
    fn depth_places_each_voice_at_its_own_delay_inside_the_documented_range() {
        let mut buf = impulse(4096);
        apply(&mut buf, 0.0, 1.0, 4, 1.0, RATE);
        let (centre, sweep) = (BASE_SECONDS * RATE, SWEEP_SECONDS * RATE);
        let landed = arrivals(&buf);
        for voice in ensemble(4, 0.0, RATE) {
            let at = (centre + sweep * voice.phase.sin()).floor() as usize;
            assert!(
                landed.contains(&at) || landed.contains(&(at + 1)),
                "no copy at {at}; energy is at {landed:?}"
            );
        }
        let (first, last) = (landed[0] as f32, landed[landed.len() - 1] as f32);
        assert!(
            first >= centre - sweep - 1.0,
            "nothing arrives before the range"
        );
        assert!(last <= centre + sweep + 1.0, "nor after it");
        assert!(
            last - first > sweep,
            "and they are spread across most of it"
        );

        // Two voices, so the left channel carries the first one alone and
        // nothing else. That is what pins the *direction* the sweep is applied
        // in: a spread measured over both channels is very nearly its own
        // mirror image, so it cannot tell `centre + sweep` from
        // `centre - sweep`, and one voice on one side can.
        let mut pair = impulse(4096);
        apply(&mut pair, 0.0, 1.0, 2, 1.0, RATE);
        let alone = &ensemble(2, 0.0, RATE)[0];
        let at = (centre + sweep * alone.phase.sin()).floor() as usize;
        let caught = pair.l[at] + pair.l[at + 1];
        assert!(caught > 0.9, "the copy is not at {at}: {caught} of it is");
    }

    /// The sweep is a sweep. Probed half a cycle apart a copy comes back at a
    /// very different delay, and probed a whole cycle apart it is home again.
    ///
    /// The cycle is **that voice's own**, not the written rate: the voices are
    /// deliberately pulled a little either side of it, so a period taken from
    /// `rate` would be a period none of them actually has.
    #[test]
    fn a_copy_sweeps_through_its_delays_and_is_back_after_one_of_its_cycles() {
        let hz = 4.0;
        // Two voices, so the left channel is the first one alone and the peak
        // found below belongs to one sweep rather than to a sum of them.
        let period = (TAU / ensemble(2, hz, RATE)[0].step).round() as usize;
        let mut buf = Stereo::silence(period + 4096);
        for probe in [0, period / 4, period / 2, period] {
            buf.l[probe] = 1.0;
            buf.r[probe] = 1.0;
        }
        apply(&mut buf, hz, 1.0, 2, 1.0, RATE);
        let loudest = |from: usize| {
            (from..from + 2048)
                .max_by(|a, b| buf.l[*a].abs().total_cmp(&buf.l[*b].abs()))
                .expect("a window to search")
                - from
        };
        let (start, middle, end) = (loudest(0), loudest(period / 2), loudest(period));
        assert!(
            start.abs_diff(end) <= 4,
            "a cycle on, back where it began: {start} then {end}"
        );
        assert!(
            start.abs_diff(middle) > 100,
            "and half a cycle on, somewhere else: {start} then {middle}"
        );

        // A quarter cycle on is somewhere else again, so the sweep is
        // visiting its delays rather than jumping between two of them.
        let quarter = loudest(period / 4);
        assert!(
            quarter.abs_diff(start) > 50 && quarter.abs_diff(middle) > 50,
            "a quarter cycle is its own place: {start}, {quarter}, {middle}"
        );
    }

    /// The blend, the way every other effect here states it: half way across
    /// is the arithmetic mean of the two ends, exactly.
    #[test]
    fn mix_blends_wet_against_dry() {
        let dry = tone(220.0);
        let mut wet = dry.clone();
        apply(&mut wet, 0.7, 0.6, 3, 1.0, RATE);
        let mut half = dry.clone();
        apply(&mut half, 0.7, 0.6, 3, 0.5, RATE);
        for i in 0..dry.frames() {
            let expected = (dry.l[i] + wet.l[i]) * 0.5;
            assert!((half.l[i] - expected).abs() < 1e-5, "left at {i}");
            let expected = (dry.r[i] + wet.r[i]) * 0.5;
            assert!((half.r[i] - expected).abs() < 1e-5, "right at {i}");
        }
    }

    #[test]
    fn the_tail_is_the_deepest_read_and_nothing_more() {
        assert!((tail_seconds(0.0) - BASE_SECONDS).abs() < 1e-9);
        assert!((tail_seconds(1.0) - (BASE_SECONDS + SWEEP_SECONDS)).abs() < 1e-9);
        assert_eq!(tail_seconds(9.0), tail_seconds(1.0), "clamped");
        assert_eq!(tail_seconds(f32::NAN), tail_seconds(0.0));
        assert!(tail_seconds(1.0) < 0.03, "a chorus is not a room");
    }
}
