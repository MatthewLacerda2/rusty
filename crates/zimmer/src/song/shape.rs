//! Turning a rendered song into one of the length that was asked for.
//!
//! Two steps, in this order, both after the master limiter:
//!
//! 1. **Length.** Truncate or pad the buffer to the target. A truncation is
//!    always faded over a few milliseconds — a buffer cut at an arbitrary sample
//!    ends mid-waveform, and that is a click, which is more audible than
//!    anything the fade removes.
//! 2. **Fades.** The recipe's own, over whatever length step 1 settled on.
//!
//! Fades come after limiting rather than before because a fade is a decision
//! about the level of the whole piece: limiting afterwards would drag the tail
//! back up and undo it.
//!
//! **One length step comes before the limiter instead**, and [`limit`] is
//! where it happens: a `wrap` tail sums the ring-out back onto the start, and
//! a sum can overshoot where neither of its halves did. Limiting first and
//! folding afterwards would hand out a buffer that clips — the one promise the
//! limiter exists to keep.

use super::Song;
use super::clock::Clock;
use super::timing::{Fade, Fit, FitMode, Tail};
use crate::core::RATE;
use crate::error::SynthError;
use crate::fx::limiter;
use crate::stereo::Stereo;

/// How long a cut is faded over so it does not click, in seconds.
///
/// Short enough to be inaudible as a fade, long enough to be inaudible as an
/// edge — about a wavelength at the bottom of hearing.
pub(super) const SEAM: f32 = 0.02;

/// The master limiter, after the one step of `tail` that has to come before
/// it — see the module doc.
///
/// Under `wrap` the song is folded at its loop point and then limited as the
/// loop it now is, so neither step leaves a seam; every other song is limited
/// as it stands, exactly as it always was.
pub(crate) fn limit(
    song: &Song,
    buf: &mut Stereo,
    arrangement_end: usize,
) -> Result<(), SynthError> {
    if song.tail() != Tail::Wrap {
        limiter::apply(buf, RATE);
        return Ok(());
    }
    // Under a `stretch` fit the loop point is the target itself: the tempo
    // was moved so the passes land there, and rounding the two separately can
    // leave them a sample apart — which would have `shape` cut, and fade, the
    // very end of a file that was meant to come round without a seam.
    let at = fitted(song).map_or(arrangement_end, |(_, seconds)| samples(seconds));
    wrap(buf, at)?;
    limiter::apply_looped(buf, RATE);
    Ok(())
}

/// Cuts `buf` at `at` and sums everything past the cut back onto the start,
/// so the file is exactly `at` long and plays round without a seam.
///
/// Refuses a tail longer than the loop rather than folding it twice: a tail
/// still ringing the second time round is a recipe that does not loop, and
/// saying so is the useful answer.
fn wrap(buf: &mut Stereo, at: usize) -> Result<(), SynthError> {
    let overhang = buf.frames().saturating_sub(at);
    if overhang > at {
        return Err(SynthError::WrapOverhang {
            overhang: overhang as f32 / RATE,
            length: at as f32 / RATE,
        });
    }
    buf.each(|channel| {
        let (head, past) = channel.split_at_mut(at.min(channel.len()));
        for (sample, tail) in head.iter_mut().zip(past.iter()) {
            *sample += tail;
        }
    });
    buf.resize(at);
    Ok(())
}

/// Applies `tail`, `fit` and `fade` to a rendered buffer, in that order.
///
/// `arrangement_end` is where the last beat falls, which is the only length
/// the buffer itself cannot tell you: past that point everything is ring-out.
///
/// Length and level are both properties of the *piece*, so every step here is
/// the same step on each channel — a fade that reached one side sooner than
/// the other would be a pan nobody asked for.
pub(crate) fn shape(song: &Song, buf: &mut Stereo, arrangement_end: usize) {
    if song.tail() == Tail::Exact {
        resize(buf, arrangement_end);
    }
    if let Some((_, seconds)) = fitted(song) {
        resize(buf, samples(seconds));
    }
    let fade = song.fade.unwrap_or_default();
    if !fade.is_silent_about_everything() {
        buf.each(|channel| apply_fade(channel, fade));
    }
}

/// The clock and arrangement this song has to be rendered at to come out at
/// its target length, and the number of passes that takes.
///
/// `Loop` and `Once` keep the written tempo and change how many times the
/// arrangement plays; `Stretch` keeps one whole number of passes and changes
/// the tempo — all of it, map and all, by one factor. The caller renders the result and then calls [`shape`], which is
/// what actually lands it on the target sample.
pub(crate) fn plan(song: &Song) -> (Clock, u32) {
    // Pinned sections decide the tempo stretch by stretch, a `stretch` fit's
    // end among them, and are played once: `validate` refuses them under a
    // `loop`, and `once` pads after the last as it always did.
    if let Some(landings) = super::anchor::landings(song) {
        return (Clock::anchored(song, &landings), 1);
    }
    let written = Clock::written(song, 1);
    let Some((fit, seconds)) = fitted(song) else {
        return (written, 1);
    };
    let once = written.seconds(song.arrangement_beats());
    if once <= 0.0 {
        return (written, 1);
    }
    match fit.mode {
        // Ceil, so the buffer is at least as long as the target and `shape`
        // only ever has to cut. Padding a loop with silence would be a gap in
        // the middle of a bed.
        FitMode::Loop => {
            let passes = passes(seconds / once, f32::ceil);
            (Clock::written(song, passes), passes)
        }
        FitMode::Once => (written, 1),
        FitMode::Stretch => {
            let passes = passes(seconds / once, f32::round);
            (Clock::stretched(song, passes, seconds), passes)
        }
    }
}

/// How far off the written tempo `stretch` would have to go, as a fraction.
///
/// Its own function because the refusal wants the number: "it would need
/// 137 bpm" is actionable, and "it does not fit" is not.
///
/// A song whose tempo moves is stretched by moving every tempo in it by the
/// same factor, so the fraction is the same wherever it is read — and it is
/// read at the first beat, against the `bpm` the document starts at.
pub(crate) fn stretch_ratio(song: &Song, fit: Fit, seconds: f32) -> Option<f32> {
    let once = Clock::written(song, 1).seconds(song.arrangement_beats());
    if once <= 0.0 || fit.mode != FitMode::Stretch {
        return None;
    }
    let passes = passes(seconds / once, f32::round);
    Some(Clock::stretched(song, passes, seconds).bpm() / song.bpm - 1.0)
}

/// The song's fit and the length it lands on, when it has one to land on.
///
/// A fit whose `to` nobody resolved has no length, and every piece of
/// arithmetic here treats it as absent: `validate` refuses it before a render
/// starts, so the only reader that meets one is a question asked of the
/// document without rendering it — where its sections fall, say — and the
/// honest answer there is the song as written.
pub(super) fn fitted(song: &Song) -> Option<(Fit, f32)> {
    let fit = song.fit?;
    Some((fit, fit.seconds?))
}

/// A pass count of at least one, rounded the way the caller asked.
fn passes(exact: f32, round: fn(f32) -> f32) -> u32 {
    let counted = round(exact);
    if counted.is_finite() && counted >= 1.0 {
        counted as u32
    } else {
        1
    }
}

/// Samples in `seconds`, rounded to nearest — the one place a target length
/// becomes a sample count, so `fit` and the tests agree on what 43 s is.
pub(crate) fn samples(seconds: f32) -> usize {
    (seconds * RATE).round().max(0.0) as usize
}

/// Cuts or pads `buf` to exactly `wanted` samples, fading a cut so it does not
/// click.
fn resize(buf: &mut Stereo, wanted: usize) {
    let cut = buf.frames() > wanted;
    buf.resize(wanted);
    if cut {
        buf.each(|channel| fade_out(channel, samples(SEAM)));
    }
}

/// The recipe's own fades, over the buffer as it now stands.
fn apply_fade(buf: &mut [f32], fade: Fade) {
    let rising = samples(fade.in_seconds).min(buf.len());
    for (index, sample) in buf[..rising].iter_mut().enumerate() {
        *sample *= index as f32 / rising as f32;
    }
    fade_out(buf, samples(fade.out_seconds));
}

/// Fades the last `length` samples of `buf` down to silence.
fn fade_out(buf: &mut [f32], length: usize) {
    let length = length.min(buf.len());
    if length == 0 {
        return;
    }
    let start = buf.len() - length;
    for (index, sample) in buf[start..].iter_mut().enumerate() {
        *sample *= 1.0 - (index as f32 / length as f32);
    }
}

/// The seam, by which side of it a length falls on.
///
/// [`resize`] is the one place here that behaves differently depending on a
/// comparison, and the case it has to get right is the boring one: a target
/// that is exactly the length already rendered. Fading there would put a
/// twenty-millisecond dip on the end of a song that was never cut — audible,
/// and invisible to every test that asks how long the result is.
#[cfg(test)]
mod tests {
    use super::*;

    /// A signal that is 1.0 all the way through, so a fade is the only thing
    /// that can make any sample anything else.
    fn flat(frames: usize) -> Stereo {
        Stereo::centred(vec![1.0; frames])
    }

    /// The seam, in samples — long enough that a cut has room to fade.
    fn seam() -> usize {
        samples(SEAM)
    }

    /// The fold is a sum and nothing else: the start is the start plus what
    /// rang past the cut, the rest is untouched, and nothing is faded — a
    /// wrapped file is not cut, so it has no edge to soften.
    #[test]
    fn a_wrap_sums_the_tail_onto_the_start() {
        let mut buf = Stereo {
            l: vec![1.0, 2.0, 3.0, 4.0, 10.0, 20.0],
            r: vec![-1.0, -2.0, -3.0, -4.0, 0.5, 0.25],
        };
        wrap(&mut buf, 4).expect("two frames of tail fold onto four");
        assert_eq!(buf.l, [11.0, 22.0, 3.0, 4.0]);
        assert_eq!(buf.r, [-0.5, -1.75, -3.0, -4.0]);
    }

    /// A tail exactly one loop long still folds; one frame longer is refused,
    /// with both lengths.
    #[test]
    fn a_tail_longer_than_the_loop_is_refused() {
        let mut fits = flat(8);
        assert!(wrap(&mut fits, 4).is_ok());
        assert_eq!(fits, Stereo::centred(vec![2.0; 4]));
        let mut rings_on = flat(9);
        assert_eq!(
            wrap(&mut rings_on, 4),
            Err(SynthError::WrapOverhang {
                overhang: 5.0 / RATE,
                length: 4.0 / RATE,
            })
        );
    }

    #[test]
    fn a_cut_is_faded_so_it_does_not_click() {
        let mut buf = flat(seam() * 4);
        resize(&mut buf, seam() * 2);
        assert_eq!(buf.frames(), seam() * 2);
        assert_eq!(buf.l[0], 1.0, "the front of it is untouched");
        assert!(buf.l[seam() * 2 - 1] < 0.01, "and the seam runs to silence");
        assert_eq!(buf.l, buf.r, "both sides fade together");
    }

    #[test]
    fn a_target_that_is_already_the_length_changes_nothing() {
        let mut buf = flat(seam() * 4);
        resize(&mut buf, seam() * 4);
        assert_eq!(buf, flat(seam() * 4), "nothing was cut, so nothing fades");
    }

    #[test]
    fn a_longer_target_is_padded_with_silence_and_not_faded() {
        let mut buf = flat(seam());
        resize(&mut buf, seam() * 2);
        assert_eq!(buf.frames(), seam() * 2);
        assert!(
            buf.l[..seam()].iter().all(|s| *s == 1.0),
            "the music is left alone"
        );
        assert!(buf.l[seam()..].iter().all(|s| *s == 0.0), "and padded");
    }
}
