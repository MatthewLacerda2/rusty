//! Freeverb.
//!
//! Jezar's **Freeverb** is the public-domain reverb everyone reaches for at this
//! scale: eight parallel damped comb filters (the dense early build-up) feeding four
//! series allpass filters (the diffusion that smears them into a tail). Around a
//! hundred lines, no tuning tables to invent, and it has sounded good in trackers and
//! game audio for twenty-five years — exactly the "standard, proven algorithm"
//! bar this layer is held to.
//!
//! The comb/allpass lengths are the original prime-ish tunings, chosen at 44.1 kHz
//! and rescaled if a bake ever runs at another rate. `size` sets the comb feedback
//! (room size), `damp` how fast the tail loses its highs, `mix` the wet/dry blend.
//!
//! ## Both halves of it
//!
//! Freeverb is a **stereo** algorithm and always was: two banks of the same
//! twelve filters, the right one's delay lines a fixed [`STEREO_SPREAD`]
//! longer than the left's. That offset is the whole trick — the same room, but
//! its reflections arrive at the two ears at times that never quite line up,
//! which is what a room actually does and what a copy of one channel into the
//! other can never be. This crate carried only the left bank while it was
//! mono; restoring the right one is restoring a constant.
//!
//! Both banks are fed the **mono fold-down** of the input, which is what the
//! original does with its two inputs and what a physical send is: one room,
//! everything in front of it going in, the width appearing on the way out
//! rather than being carried in. A signal already dead centre therefore feeds
//! it at exactly its own level, and its left channel comes back the tail this
//! reverb has always produced.
//!
//! ## Pre-delay
//!
//! The one control added to Jezar's design. Freeverb's combs start answering
//! on the very sample a sound arrives, so its tail lands on the transient and
//! smears it: a snare loses its crack in a big room. A `predelay` holds the
//! send back that long before the room hears it — the gap a large hall has
//! before its first wall answers — and the dry sound arrives alone. It is a
//! plain line on the send, and none at all when it is zero, which is every
//! recipe written before it existed.
//!
//! Jezar's `width` control, which bleeds each wet channel into the other, is
//! not carried across. At its default it is a no-op, and every setting other
//! than the default narrows the image the spread above just created — a knob
//! whose whole range is *less of the effect* is not a knob.

use crate::stereo::Stereo;

/// The eight parallel comb-filter lengths, in samples at 44.1 kHz.
const COMB_LENGTHS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
/// The four series allpass lengths, in samples at 44.1 kHz.
const ALLPASS_LENGTHS: [usize; 4] = [556, 441, 341, 225];
/// How much longer every delay line in the right bank is, in samples at
/// 44.1 kHz.
///
/// Jezar's `stereospread`, unchanged. Half a millisecond: far too short to
/// hear as an echo, and comfortably long enough that the two banks' comb
/// resonances land on different frequencies, which is what makes the tail
/// arrive from a width rather than from a point.
const STEREO_SPREAD: usize = 23;
/// The rate the tunings above were chosen at.
///
/// Equal to [`crate::SAMPLE_RATE`] today, so the rescale below is a no-op and
/// the reverb is exactly Freeverb's original voicing. It is kept as a separate
/// number rather than collapsed into one, because the two mean different
/// things: the render rate is a delivery decision, and *these* are the lengths
/// that make this particular room sound like a room. Collapsing them would
/// make a later change to the render rate silently retune the reverb.
const TUNED_RATE: f32 = 44_100.0;
/// Input scaling, so eight summed combs stay in range.
const FIXED_GAIN: f32 = 0.015;
/// `size` maps to comb feedback as `size × SCALE + OFFSET`.
const ROOM_SCALE: f32 = 0.28;
const ROOM_OFFSET: f32 = 0.7;
/// `damp` maps to the comb's internal lowpass coefficient by this scale.
const DAMP_SCALE: f32 = 0.4;
/// The allpass feedback Freeverb fixes at 0.5.
const ALLPASS_FEEDBACK: f32 = 0.5;
/// The longest pre-delay, in seconds. Past half a second the room stops
/// sounding late and starts sounding like an echo, which is what `delay` is.
pub(crate) const MAX_PREDELAY: f32 = 0.5;

/// One damped comb filter: a delay line with a one-pole lowpass in its feedback
/// path, which is what makes the tail darken as it decays.
struct Comb {
    line: Vec<f32>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    fn step(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.line[self.index];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.line[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.line.len();
        output
    }
}

/// One allpass filter: passes every frequency at equal level but scrambles their
/// phases, turning the combs' discrete echoes into a smooth tail.
struct Allpass {
    line: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len.max(1)],
            index: 0,
        }
    }

    fn step(&mut self, input: f32) -> f32 {
        let buffered = self.line[self.index];
        self.line[self.index] = input + buffered * ALLPASS_FEEDBACK;
        self.index = (self.index + 1) % self.line.len();
        buffered - input
    }
}

/// One side of the room: the eight combs and four allpasses one channel's wet
/// signal passes through.
struct Bank {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

impl Bank {
    /// The bank for one side, its delay lines `offset` samples longer than the
    /// tuning table says and rescaled by `scale` if the render rate ever moves
    /// off the one the tunings were chosen at.
    fn new(offset: usize, scale: f32) -> Self {
        let scaled = |len: usize| (((len + offset) as f32 * scale).round() as usize).max(1);
        Self {
            combs: COMB_LENGTHS.iter().map(|l| Comb::new(scaled(*l))).collect(),
            allpasses: ALLPASS_LENGTHS
                .iter()
                .map(|l| Allpass::new(scaled(*l)))
                .collect(),
        }
    }

    /// One sample of send in, one sample of tail out.
    fn step(&mut self, send: f32, feedback: f32, damp: f32) -> f32 {
        let mut wet: f32 = self
            .combs
            .iter_mut()
            .map(|c| c.step(send, feedback, damp))
            .sum();
        for ap in self.allpasses.iter_mut() {
            wet = ap.step(wet);
        }
        wet
    }
}

/// Apply Freeverb to `buf` in place, the room hearing it `predelay` seconds
/// late.
pub(crate) fn apply(buf: &mut Stereo, size: f32, damp: f32, predelay: f32, mix: f32, rate: f32) {
    room(buf, None, [size, damp, predelay], mix, rate);
}

/// Apply Freeverb to `buf` in place, with the room hearing `sent` rather than
/// `buf` itself: the dry side is `buf`'s and the tail is `sent`'s.
///
/// What a song's per-track `send` needs — the mix stays whole while each part
/// reaches the room at its own amount. Past the end of `sent` the room hears
/// silence.
pub(crate) fn apply_sent(
    buf: &mut Stereo,
    sent: &Stereo,
    size: f32,
    damp: f32,
    predelay: f32,
    mix: f32,
    rate: f32,
) {
    room(buf, Some(sent), [size, damp, predelay], mix, rate);
}

/// The one Freeverb loop — `room` is the size, the damping and the
/// pre-delay — fed from `sent` when there is one and from `buf`
/// otherwise — so a reverb heard through a send is the same arithmetic as one
/// that is not, and a send of everything is the same samples.
fn room(
    buf: &mut Stereo,
    sent: Option<&Stereo>,
    [size, damp, predelay]: [f32; 3],
    mix: f32,
    rate: f32,
) {
    let scale = if rate > 0.0 { rate / TUNED_RATE } else { 1.0 };
    let mut late = Late::new(predelay, rate);
    let mut left = Bank::new(0, scale);
    let mut right = Bank::new(STEREO_SPREAD, scale);

    let feedback = size.clamp(0.0, 1.0) * ROOM_SCALE + ROOM_OFFSET;
    let damp = damp.clamp(0.0, 1.0) * DAMP_SCALE;
    let mix = mix.clamp(0.0, 1.0);
    for (i, (l, r)) in buf.l.iter_mut().zip(buf.r.iter_mut()).enumerate() {
        let (in_l, in_r) = sent.map_or((*l, *r), |sent| sent.frame(i));
        // One send, taken before either side is overwritten — the room hears
        // the mix, not the half of it that happens to be on this channel.
        let send = late.step((in_l + in_r) * 0.5 * FIXED_GAIN);
        let wet_l = left.step(send, feedback, damp);
        let wet_r = right.step(send, feedback, damp);
        *l = *l * (1.0 - mix) + wet_l * mix;
        *r = *r * (1.0 - mix) + wet_r * mix;
    }
}

/// The pre-delay: a plain line on the send, or nothing at all when there is
/// none to hold — so a room with no pre-delay is the arithmetic it always was.
struct Late {
    line: Vec<f32>,
    index: usize,
}

impl Late {
    fn new(predelay: f32, rate: f32) -> Self {
        let len = (predelay_seconds(predelay) * rate).round() as usize;
        Self {
            line: vec![0.0; len],
            index: 0,
        }
    }

    /// One sample of send in, the one sent `predelay` ago out.
    fn step(&mut self, send: f32) -> f32 {
        let Some(slot) = self.line.get_mut(self.index) else {
            return send;
        };
        let out = std::mem::replace(slot, send);
        self.index = (self.index + 1) % self.line.len();
        out
    }
}

/// A pre-delay as the room takes it: clamped to `0..=`[`MAX_PREDELAY`], and
/// none for a value that is not a number.
pub(crate) fn predelay_seconds(predelay: f32) -> f32 {
    if predelay.is_finite() {
        predelay.clamp(0.0, MAX_PREDELAY)
    } else {
        0.0
    }
}

/// How long the tail rings after the dry signal stops, in seconds — what the
/// renderer pads the buffer by so the reverb is not cut off.
///
/// Read off the feedback rather than guessed from `size`: the time the
/// slowest comb takes to fall 60 dB (RT60, the decay a room is said to have
/// stopped ringing at), plus one pass through the allpasses behind it. The
/// damping only ever shortens that — it is a lowpass in the loop, and at the
/// bottom of the spectrum it passes everything — so the low end is the tail
/// this has to cover. The straight line this replaced (#1123) gave a full-size
/// room three seconds where it rings for more than ten, and cut it while it
/// was still clearly audible; measured on a 110 Hz burst, this lands within a
/// few hundred milliseconds of the real −60 dB point at every size.
pub(crate) fn tail_seconds(size: f32) -> f32 {
    let feedback = size.clamp(0.0, 1.0) * ROOM_SCALE + ROOM_OFFSET;
    let samples = |lengths: &[usize]| lengths.iter().copied().max().unwrap_or(0) + STEREO_SPREAD;
    let loop_seconds = samples(&COMB_LENGTHS) as f32 / TUNED_RATE;
    let diffusion: usize = ALLPASS_LENGTHS.iter().sum::<usize>() + STEREO_SPREAD;
    loop_seconds * (0.001f32.ln() / feedback.ln()) + diffusion as f32 / TUNED_RATE
}

#[cfg(test)]
mod tests;
