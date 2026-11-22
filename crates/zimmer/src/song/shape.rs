//! Turning a rendered song into one of the length that was asked for.

use super::clock::Clock;
use super::timing::{Fade, Fit, FitMode, Tail};
use super::Song;
use crate::core::RATE;
use crate::error::SynthError;
use crate::fx::limiter;
use crate::stereo::Stereo;

/// Seconds a cut fades over, so its edge does not click.
pub(super) const SEAM: f32 = 0.02;

/// Folds a `wrap` tail before limiting: the sum can overshoot, so limiting first would hand out
/// clipping.
pub(crate) fn limit(
    song: &Song,
    buf: &mut Stereo,
    arrangement_end: usize,
) -> Result<(), SynthError> {
    if song.tail() != Tail::Wrap {
        limiter::apply(buf, RATE);
        return Ok(());
    }
    // A stretch fit's loop point is its target: rounding the two apart could leave `shape` cutting
    // the end.
    let at = fitted(song).map_or(arrangement_end, |(_, seconds)| samples(seconds));
    wrap(buf, at)?;
    limiter::apply_looped(buf, RATE);
    Ok(())
}

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

/// Runs after the limiter: limiting a faded tail would pull it back up.
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

pub(crate) fn plan(song: &Song) -> (Clock, u32) {
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
        // Ceil: the buffer is never short of the target, so `shape` cuts a loop and never pads it.
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

pub(crate) fn stretch_ratio(song: &Song, fit: Fit, seconds: f32) -> Option<f32> {
    let once = Clock::written(song, 1).seconds(song.arrangement_beats());
    if once <= 0.0 || fit.mode != FitMode::Stretch {
        return None;
    }
    let passes = passes(seconds / once, f32::round);
    Some(Clock::stretched(song, passes, seconds).bpm() / song.bpm - 1.0)
}

pub(super) fn fitted(song: &Song) -> Option<(Fit, f32)> {
    let fit = song.fit?;
    Some((fit, fit.seconds?))
}

fn passes(exact: f32, round: fn(f32) -> f32) -> u32 {
    let counted = round(exact);
    if counted.is_finite() && counted >= 1.0 {
        counted as u32
    } else {
        1
    }
}

pub(crate) fn samples(seconds: f32) -> usize {
    (seconds * RATE).round().max(0.0) as usize
}

fn resize(buf: &mut Stereo, wanted: usize) {
    let cut = buf.frames() > wanted;
    buf.resize(wanted);
    if cut {
        buf.each(|channel| fade_out(channel, samples(SEAM)));
    }
}

fn apply_fade(buf: &mut [f32], fade: Fade) {
    let rising = samples(fade.in_seconds).min(buf.len());
    for (index, sample) in buf[..rising].iter_mut().enumerate() {
        *sample *= index as f32 / rising as f32;
    }
    fade_out(buf, samples(fade.out_seconds));
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(frames: usize) -> Stereo {
        Stereo::centred(vec![1.0; frames])
    }

    fn seam() -> usize {
        samples(SEAM)
    }

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
