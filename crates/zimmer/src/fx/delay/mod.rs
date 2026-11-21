//! feedback echo.
//!
//! A ring buffer one echo-time long, read and written each sample: what comes out
//! is fed back in, scaled, so each repeat is quieter than the last. Cheap, and it is
//! the effect that most reliably makes a dry one-shot sound like it happened
//! *somewhere* — a slapback on a gunshot, a corridor on a footstep.
//!
//! The wet signal is blended with the dry one by `mix`, so a delay is always a
//! parallel effect, never a replacement.
//!
//! ## Two of them, and the difference is where the repeats land
//!
//! [`apply`] is the plain one: one line, one channel, no knowledge that there
//! is another side. Run over both through [`Stereo::each`] it gives a centred
//! source two identical echoes, which places that sound in **time** and never
//! in width — correct, and invisible.
//!
//! [`ping_pong`] is the stereo one, and it cannot be reached that way: the
//! left line's output is what feeds the right line, so the two sides are the
//! same delay rather than two of it. The input enters on one side only, which
//! is what makes the first repeat land left, the second right, and the tail
//! walk across the field. That cross-feed is the whole of the difference —
//! same `time`, same `feedback`, same tail length.
//!
//! The send is the **mono fold-down** of the input, for the reason
//! [`reverb`](super::reverb) gives for its own: a ping-pong is one device
//! everything goes into, and the width appears on the way out rather than
//! being carried in. A signal already dead centre therefore feeds it at
//! exactly its own level.

use super::Tempo;
use crate::stereo::Stereo;

/// How far apart the repeats are: a fixed number of seconds, or a number of
/// beats of whatever tempo is playing.
///
/// The second is why a line is not always one length. Under a tempo map a
/// beat lasts a different time at different points of the piece, and a delay
/// timed in beats follows it, sample by sample: each echo is read from as far
/// back as the beat is long *now*. The line is sized for the longest beat
/// anywhere, so a tempo that slows never reads past what was kept. A spacing
/// that moves jumps a whole sample at a time — an echo of a ramp repeats or
/// skips one now and then, which is far below anything a repeat a tenth of a
/// second away can be heard to do.
#[derive(Clone, Copy)]
pub(crate) enum Spacing<'a> {
    /// The same number of seconds throughout — `time`.
    Seconds(f32),
    /// This many beats of `tempo` — `beats`.
    Beats(f32, &'a dyn Tempo),
}

impl<'a> Spacing<'a> {
    /// The spacing a delay's `time` and `beats` ask for: beats when there are
    /// any — validation refuses both at once — and seconds otherwise.
    pub(crate) fn new(time: f32, beats: Option<f32>, tempo: &'a dyn Tempo) -> Self {
        match beats {
            Some(beats) => Self::Beats(beats, tempo),
            None => Self::Seconds(time),
        }
    }

    /// The longest one echo is anywhere, in seconds — the line's length, and
    /// what [`tail_seconds`] measures the ring-out by.
    pub(crate) fn longest(self) -> f32 {
        match self {
            Self::Seconds(time) => time,
            Self::Beats(beats, tempo) => beats * tempo.longest_beat(),
        }
    }

    /// Samples one echo spans at `frame`, on a line `cap` samples long.
    fn at(self, frame: usize, cap: usize, rate: f32) -> usize {
        match self {
            Self::Seconds(_) => cap,
            Self::Beats(beats, tempo) => {
                let len = (beats * tempo.beat_seconds(frame) * rate).round();
                (len as usize).clamp(1, cap)
            }
        }
    }
}

/// Apply a feedback delay to `buf` in place.
///
/// `spacing` is how far apart the echoes are, `feedback` how much of each echo
/// feeds the next (clamped below 1 so the tail always dies), `mix` the wet/dry
/// blend.
pub(crate) fn apply(buf: &mut [f32], spacing: Spacing<'_>, feedback: f32, mix: f32, rate: f32) {
    line(buf, None, spacing, feedback, mix, rate);
}

/// [`apply`], with the echoes fed from `sent` rather than from `buf`: the dry
/// side is `buf`'s and the repeats are `sent`'s. Past the end of `sent` the
/// line hears silence. What a song's per-track `send` needs; see
/// [`crate::fx::reverb::apply_sent`].
pub(crate) fn apply_sent(
    buf: &mut [f32],
    sent: &[f32],
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    line(buf, Some(sent), spacing, feedback, mix, rate);
}

/// The one feedback-delay loop, fed from `sent` when there is one and from
/// `buf` otherwise, so the two are the same arithmetic.
///
/// The line is written at `i` and read `len` behind it. At a fixed spacing
/// `len` is the whole line, so the slot read is the one about to be written —
/// the plain ring buffer, sample for sample.
fn line(
    buf: &mut [f32],
    sent: Option<&[f32]>,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    let Some(cap) = line_length(spacing.longest(), buf.len(), rate) else {
        return;
    };
    let feedback = feedback.clamp(0.0, 0.95);
    let mix = mix.clamp(0.0, 1.0);
    let mut line = vec![0.0f32; cap];
    for (i, s) in buf.iter_mut().enumerate() {
        let input = sent.map_or(*s, |sent| sent.get(i).copied().unwrap_or(0.0));
        let write = i % cap;
        let echo = line[(write + cap - spacing.at(i, cap, rate)) % cap];
        line[write] = input + echo * feedback;
        *s = *s * (1.0 - mix) + echo * mix;
    }
}

/// Apply a ping-pong delay to `buf` in place: the first repeat on the left,
/// the next on the right, and so on down the tail.
///
/// Two lines, cross-fed. The mono send enters the left one, the left one's
/// output feeds the right one, and the right one's output feeds the left —
/// so an echo lands on alternate sides at exactly the spacing [`apply`] would
/// have put them on both, and decays by `feedback` per repeat exactly as it
/// would have there. Which is why [`tail_seconds`] is the same arithmetic: a
/// tail is the same length however the repeats are placed.
pub(crate) fn ping_pong(
    buf: &mut Stereo,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    crossed(buf, None, spacing, feedback, mix, rate);
}

/// [`ping_pong`], with the repeats fed from `sent` rather than from `buf`, as
/// [`apply_sent`] is to [`apply`].
pub(crate) fn ping_pong_sent(
    buf: &mut Stereo,
    sent: &Stereo,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    crossed(buf, Some(sent), spacing, feedback, mix, rate);
}

/// The one ping-pong loop, fed from `sent` when there is one, and read the
/// way [`line`] reads its one line.
fn crossed(
    buf: &mut Stereo,
    sent: Option<&Stereo>,
    spacing: Spacing<'_>,
    feedback: f32,
    mix: f32,
    rate: f32,
) {
    let Some(cap) = line_length(spacing.longest(), buf.frames(), rate) else {
        return;
    };
    let feedback = feedback.clamp(0.0, 0.95);
    let mix = mix.clamp(0.0, 1.0);
    let mut left = vec![0.0f32; cap];
    let mut right = vec![0.0f32; cap];
    for i in 0..buf.frames() {
        let (dry_l, dry_r) = (buf.l[i], buf.r[i]);
        let (in_l, in_r) = sent.map_or((dry_l, dry_r), |sent| sent.frame(i));
        let write = i % cap;
        let read = (write + cap - spacing.at(i, cap, rate)) % cap;
        let (echo_l, echo_r) = (left[read], right[read]);
        left[write] = (in_l + in_r) * 0.5 + echo_r * feedback;
        right[write] = echo_l * feedback;
        buf.l[i] = dry_l * (1.0 - mix) + echo_l * mix;
        buf.r[i] = dry_r * (1.0 - mix) + echo_r * mix;
    }
}

/// How many samples one echo is, or `None` for a delay there is no point
/// running.
///
/// A non-positive or non-finite `time` is not a delay at all, and an echo
/// further away than eight times the signal itself would never be heard —
/// skipping beats allocating a line nobody reads. Both answers are the same
/// for one line or two, which is why they are asked here rather than twice.
fn line_length(time: f32, frames: usize, rate: f32) -> Option<usize> {
    if !time.is_finite() || time <= 0.0 {
        return None;
    }
    let len = ((time * rate).round() as usize).max(1);
    (len < frames.max(1) * 8).then_some(len)
}

/// How long the echoes keep ringing after the dry signal stops, in seconds — what
/// the renderer pads the buffer by so the tail is not cut off mid-repeat.
pub(crate) fn tail_seconds(time: f32, feedback: f32) -> f32 {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    let feedback = feedback.clamp(0.0, 0.95);
    if feedback <= 0.0 {
        return time;
    }
    // Echo `k` lands at `k · time` at `feedback^(k-1)`; the last one kept is
    // the last still above −60 dB. The chain's own cap bounds the product.
    let repeats = (0.001f32.ln() / feedback.ln()).ceil() + 1.0;
    time * repeats
}

#[cfg(test)]
mod tests;
