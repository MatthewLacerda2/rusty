//! the colour of noise: how a draw's energy is spread across the spectrum.
//!
//! White noise carries equal energy per **hertz**, so half of it sits in the
//! top octave and nearly all of it above 1 kHz — which is why it is a *hiss*
//! and why almost nothing in the physical world is white. Wind, surf, rain,
//! room tone, the body of a snare and the low half of an impact all fall away
//! with frequency, and the two slopes worth naming are −3 dB per octave
//! (**pink**, equal energy per octave) and −6 (**brown**, an integrated white
//! draw).
//!
//! A lowpass over white is not a substitute and that is the whole reason this
//! module exists: a filter changes the slope only *at its cutoff* and leaves
//! everything below it flat, so what comes out is a hiss with the top taken
//! off rather than a different colour. Measured over the octave centres from
//! 62.5 Hz to 8 kHz, a single one-pole over white fits −2.4 to −5.9 dB per
//! octave depending on where the cutoff is put, and it misses that fit by as
//! much as **3.9 dB** band to band — it is never a straight line, which is the
//! one thing a colour is.
//!
//! ## Pink is a bank of one-poles, and it was measured before it was chosen
//!
//! Three of them, Paul Kellett's economical pinking filter: the poles are
//! staggered about three octaves apart so that each one's −6 dB skirt is
//! handed over to the next before it can get steep, and the sum of the three
//! is a −3 dB line. Measured against the same probe as above: **−3.00 dB per
//! octave with 1.1 dB of ripple** across ten octaves from 31 Hz to 16 kHz, and
//! −2.98 with 0.56 dB of ripple over the seven octaves from 125 Hz up, where
//! the ear actually is.
//!
//! Two more elaborate generators were built and measured beside it, because
//! reaching for one before knowing the simple thing had failed is how a crate
//! this size grows a DSP department. Kellett's seven-term refinement fits
//! −3.00 with 0.67 dB of ripple, and **Voss-McCartney** — a binary tree of
//! independently redrawn rows — fits −2.97 with 0.67. Neither is audibly
//! straighter than a ripple already inside a decibel, both carry more state,
//! and Voss needs a second draw stream of its own. Three poles it is.
//!
//! ## Brown is a leaky integrator, and the leak is the whole design
//!
//! Summing white noise integrates it, which is exactly the −6 dB per octave
//! wanted. Summing it *without a leak* is a random walk: its excursion grows
//! without bound as the note gets longer, so it wanders off centre, and the DC
//! it wanders to eats the headroom of everything downstream while making no
//! sound at all. Measured over one draw: an unleaky integrator peaks at 105
//! after 16 thousand samples and at **493** after a million, and its mean sits
//! 15 away from silence. With the leak it peaks at 22.7 and 24.0 over the same
//! two lengths — bounded, and stationary, which is what makes it a *sound*
//! rather than a slow ramp.
//!
//! [`BROWN_LEAK`] puts the pole at about 35 Hz, below the bottom of anything a
//! recipe is writing for, and buys −5.89 dB per octave with 0.37 dB of ripple
//! from 125 Hz up. Leaking less measures marginally straighter and drifts
//! visibly more; leaking more flattens the slope to −5.5.
//!
//! ## Every colour arrives at the same level
//!
//! Each is scaled so its RMS matches white's, which is the measurable stand-in
//! for *the same volume* and means changing a recipe's colour changes its
//! character and not its fader.
//!
//! **It cannot be literal loudness matching**, and it is worth saying why
//! rather than letting a reader assume it is. The ear weights the top octaves
//! far above the bottom, so brown at equal RMS is heard well over a dozen
//! decibels quieter than white, and matching *that* would ask for more level
//! than a signal can carry. Equal RMS is the closest honest target.
//!
//! What it costs is the **crest factor**. White here is a uniform draw and
//! peaks at exactly 1.0; pink and brown are sums of many draws and so are
//! Gaussian-ish, and over a two-second note they measure 2.4 and 2.1 at the
//! same RMS. That is what noise of those colours is, the peak is not a defect
//! to be clipped out of it, and the master limiter every bake passes through
//! is where this crate has always answered for peaks.
//!
//! Measured end to end, off a rendered note rather than off the filter: white
//! −0.05 dB per octave, pink −3.06, brown −5.97, at 0.07 and 0.15 dB from
//! white's level. `zimmer/tests/suite/patch/noise.rs` is where those are asserted.

use crate::patch::NoiseColor;

/// One channel's colouring, and the state it carries between samples.
///
/// Per channel rather than per source, which is the deliberate part: the
/// [noise source](super) draws its two sides independently, and running a
/// separate filter over each keeps them that way — a linear filter over two
/// uncorrelated inputs gives two uncorrelated outputs. A single shared filter,
/// or one draw coloured and copied, would collapse rain and wind and room tone
/// back to a point in space, which is the one thing this source exists not to
/// be.
pub(crate) enum Coloring {
    /// White is the identity: the draw is already the signal, and this arm
    /// exists so a `white` source is the samples it always was rather than the
    /// samples a filter set to do nothing produces.
    White,
    /// Three staggered one-poles, summed with a little of the raw draw.
    Pink {
        /// Each pole's running state, in [`PINK_POLES`] order.
        poles: [f32; PINK_POLES.len()],
    },
    /// A leaky integrator's running sum.
    Brown {
        /// How far the walk currently is from silence.
        level: f32,
    },
}

/// The three poles of the pink filter: `(decay, feed)` each, applied as
/// `pole = decay × pole + white × feed`.
///
/// Kellett's economical coefficients. The decays are what stagger the corner
/// frequencies; the feeds are what make the three skirts add up to a straight
/// line rather than a staircase.
const PINK_POLES: [(f32, f32); 3] = [
    (0.99765, 0.099_046_0),
    (0.96300, 0.296_516_4),
    (0.57000, 1.052_691_3),
];

/// How much of the raw draw is added back on top of the three poles — the term
/// that keeps the top octave from falling away faster than −3 dB.
const PINK_DIRECT: f32 = 0.1848;

/// What the pink sum is multiplied by to land on white's RMS. Measured over a
/// million samples of the filter's own output, not derived.
const PINK_SCALE: f32 = 0.3372;

/// How much of the running sum survives each sample. Below 1, or the
/// integrator is a random walk that never comes back — see the module doc.
const BROWN_LEAK: f32 = 0.995;

/// What the integrator's output is multiplied by to land on white's RMS.
/// Measured, like [`PINK_SCALE`].
const BROWN_SCALE: f32 = 0.1009;

/// How many samples of noise a coloured filter is run over before the note
/// starts.
///
/// A filter that begins at rest begins at silence, and its slowest pole here
/// takes some 425 samples to forget that — so an unwarmed pink source would
/// fade in over the first ten milliseconds, which is precisely the stretch a
/// listener identifies a snare or an impact by. Priming it on the noise the
/// source was already making costs a few thousand hashes once per note and
/// makes sample zero as pink as sample ten thousand.
///
/// Roughly five time constants of that slowest pole, which settles it to
/// within a percent.
pub(crate) const WARMUP: usize = 2048;

impl Coloring {
    /// The filter a colour asks for, at rest.
    pub(crate) fn new(color: NoiseColor) -> Self {
        match color {
            NoiseColor::White => Self::White,
            NoiseColor::Pink => Self::Pink {
                poles: [0.0; PINK_POLES.len()],
            },
            NoiseColor::Brown => Self::Brown { level: 0.0 },
        }
    }

    /// How many samples this filter must be run over before it is making the
    /// colour it claims to. Zero for white, which has nothing to settle.
    pub(crate) fn warmup(&self) -> usize {
        match self {
            Self::White => 0,
            Self::Pink { .. } | Self::Brown { .. } => WARMUP,
        }
    }

    /// One white sample in, one coloured sample out.
    pub(crate) fn step(&mut self, white: f32) -> f32 {
        match self {
            Self::White => white,
            Self::Pink { poles } => {
                let mut sum = white * PINK_DIRECT;
                for (pole, (decay, feed)) in poles.iter_mut().zip(PINK_POLES) {
                    *pole = decay * *pole + white * feed;
                    sum += *pole;
                }
                sum * PINK_SCALE
            }
            Self::Brown { level } => {
                *level = BROWN_LEAK * *level + white;
                *level * BROWN_SCALE
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mechanism the module doc calls the whole design: fed a constant,
    /// a leaky integrator settles at `1 / (1 − leak)` and an unleaky one runs
    /// away forever. A leak of exactly 1 would pass every spectral test in
    /// this crate and fail here.
    #[test]
    fn the_brown_integrator_leaks_rather_than_running_away() {
        const { assert!(BROWN_LEAK < 1.0, "an unleaky integrator is a random walk") };
        let mut brown = Coloring::new(NoiseColor::Brown);
        let mut last = 0.0;
        for _ in 0..200_000 {
            last = brown.step(1.0);
        }
        let settled = BROWN_SCALE / (1.0 - BROWN_LEAK);
        assert!(
            (last - settled).abs() < 0.01 * settled,
            "a held input settles at {settled}, not {last}"
        );
    }

    /// And the same leak read the other way: a burst of input decays away
    /// instead of being held forever, so yesterday's DC is not still in the
    /// signal.
    #[test]
    fn the_brown_integrator_forgets_what_it_was_handed() {
        let mut brown = Coloring::new(NoiseColor::Brown);
        let struck = brown.step(1.0);
        let mut level = struck;
        for _ in 0..1000 {
            level = brown.step(0.0);
        }
        assert!(level.abs() < 0.01 * struck, "still holding {level}");
    }

    /// White is the identity and not a filter set to do nothing: every sample
    /// comes back exactly as it went in, which is what keeps a `white` source
    /// byte-identical to the one that existed before colour did.
    #[test]
    fn white_is_the_draw_itself() {
        let mut white = Coloring::new(NoiseColor::White);
        assert_eq!(white.warmup(), 0);
        for sample in [-1.0, -0.25, 0.0, 0.3, 1.0] {
            assert_eq!(white.step(sample), sample);
        }
    }

    /// Both coloured filters carry state, so both have to be primed — the
    /// number is one place and neither arm may quietly answer zero.
    #[test]
    fn every_coloured_filter_asks_to_be_warmed() {
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            assert_eq!(Coloring::new(color).warmup(), WARMUP, "{color:?}");
        }
        const { assert!(WARMUP > 4 * 425, "the slowest pink pole needs longer") };
    }

    /// Every pink pole is in play: knocking any single one out of the bank
    /// changes what comes back, so no coefficient here is decoration.
    #[test]
    fn all_three_pink_poles_reach_the_output() {
        let draws: Vec<f32> = (0..4096)
            .map(|i| ((i * 37 % 101) as f32 / 50.0) - 1.0)
            .collect();
        let run = |skip: Option<usize>| {
            let mut poles = [0.0f32; PINK_POLES.len()];
            let mut out = Vec::with_capacity(draws.len());
            for white in &draws {
                let mut sum = white * PINK_DIRECT;
                for (at, (pole, (decay, feed))) in poles.iter_mut().zip(PINK_POLES).enumerate() {
                    *pole = decay * *pole + white * feed;
                    if Some(at) != skip {
                        sum += *pole;
                    }
                }
                out.push(sum * PINK_SCALE);
            }
            out
        };
        let whole = run(None);
        let mut filter = Coloring::new(NoiseColor::Pink);
        let through: Vec<f32> = draws.iter().map(|w| filter.step(*w)).collect();
        assert_eq!(whole, through, "the bank is not the sum of its poles");
        for at in 0..PINK_POLES.len() {
            assert_ne!(run(Some(at)), whole, "pole {at} changes nothing");
        }
        const { assert!(PINK_DIRECT > 0.0, "the raw draw is part of the sum") };
    }
}
