//! The fx chain a patch, a track or the song is put through.
//!
//! Plain serde data, like [`stages`](super::stages), and split from it for a
//! reason the other stages do not have: a chain is the one part of a patch
//! that is **also written somewhere a patch is not**. A track carries one and
//! so does the song, and both read these types without ever naming a source,
//! a filter or an envelope — see [`check_chain`](super::check_chain), which
//! is the one check all three go through.
//!
//! [`crate::fx`] is where the arithmetic lives. The split is the same one
//! [`fm`](super::fm) and [`core::fm`](crate::core::fm) already make: the
//! document says what was asked for, the renderer knows how to do it.

use serde::{Deserialize, Serialize};

use super::stages::one;
use crate::level::bands;

/// The most bands one EQ may carry.
///
/// Five kinds exist and a thorough treatment is rarely more than one of each —
/// a high-pass, a shelf at either end and a couple of peaks. Past that a
/// recipe is assembling a filter bank one band at a time, which is a different
/// tool from the one this is; the cap is the argument [`MAX_OSCS`](super::stages::MAX_OSCS) makes about
/// a stack, applied to arithmetic that runs over every sample of every note.
pub const MAX_EQ_BANDS: usize = 8;

/// What one EQ band does to the spectrum.
///
/// Five, and they are the five a mix is actually made of. Two remove an end of
/// the range outright and have no amount to ask for; three change how much of
/// a region there is, and so read their `gain_db`.
/// Spelled the way [`FilterKind`](super::stages::FilterKind) is, and its doc carries the argument: one
/// word, no underscore, on both surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EqKind {
    /// Everything below the frequency goes — the first move any engineer makes
    /// on anything that is not the bass.
    HighPass,
    /// Everything below the frequency moves by `gain_db`, together.
    LowShelf,
    /// A bump or a dip centred on the frequency, `q` wide. Taking 250 Hz out
    /// of a pad and keeping the pad is this one.
    Peak,
    /// Everything above the frequency moves by `gain_db`, together. Air.
    HighShelf,
    /// Everything above the frequency goes.
    LowPass,
}

impl EqKind {
    /// Whether this kind reads `gain_db` at all.
    ///
    /// The two pass filters do not: they remove an end of the range, and there
    /// is no amount of *gone* to ask for. That distinction is what lets the
    /// zero-gain bypass be a rule rather than a special case — `gain_db: 0.0`
    /// means "this band does nothing" for the three that read it, and would
    /// silently disable the two that do not.
    pub fn takes_gain(self) -> bool {
        matches!(self, Self::LowShelf | Self::Peak | Self::HighShelf)
    }

    /// Where a band of this kind sits when the recipe does not say, in Hz.
    ///
    /// **The two numbers the bake report splits its bands at**, and that is
    /// the point rather than a coincidence. A report reading `low 61%` is a
    /// finding about the energy under 250 Hz, so the band that treats it
    /// should be reachable without the reader converting anything: the three
    /// kinds that work on the bottom default to the low crossover and the two
    /// that work on the top default to the high one. So
    /// `{ "kind": "lowshelf", "gain_db": -3 }` reads as *take 3 dB off the
    /// thing the report just called low*.
    pub fn crossover(self) -> f32 {
        match self {
            Self::HighPass | Self::LowShelf | Self::Peak => bands::LOW_HZ,
            Self::HighShelf | Self::LowPass => bands::HIGH_HZ,
        }
    }
}

/// One band of an [`Fx::Eq`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EqBand {
    /// What this band does to the spectrum.
    pub kind: EqKind,
    /// Where it acts, in Hz — a corner for the pass filters and the shelves, a
    /// centre for a peak.
    ///
    /// Absent means [`EqKind::crossover`]: the boundary the bake report
    /// already draws around the end of the range this kind works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freq: Option<f32>,
    /// How much is added or taken away, in decibels — positive boosts,
    /// negative cuts.
    ///
    /// **`0.0` is a bypass.** A band at zero gain is not applied at all, so it
    /// is sample-identical to leaving it out, which is what lets a recipe list
    /// the bands it is thinking about and sweep one of them without the others
    /// colouring anything on the way past.
    ///
    /// Read only by the three kinds [`EqKind::takes_gain`] names.
    #[serde(default)]
    pub gain_db: f32,
    /// How narrow the band is; higher is narrower. Around `0.7` is the gentle,
    /// non-resonant default, `2` is a noticeable notch and `8` is a surgical
    /// one aimed at a single ringing frequency.
    ///
    /// It means the same thing for all five kinds: the pass filters and the
    /// shelves take it as the steepness of their corner, where much past `1`
    /// starts to put a resonant bump at the corner itself.
    #[serde(default = "gentle_q")]
    pub q: f32,
}

impl EqBand {
    /// Where this band acts, in Hz, with the default resolved.
    pub fn hz(&self) -> f32 {
        self.freq.unwrap_or_else(|| self.kind.crossover())
    }
}

/// One effect in the post-chain, applied in list order.
///
/// A limiter is *always* applied at bake and is deliberately not listed here —
/// it is not a choice the recipe gets to make.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fx", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fx {
    /// Feedback echo.
    Delay {
        /// Echo spacing in seconds. Absent (zero) when [`beats`](Fx::Delay::beats)
        /// says it instead — a recipe writes one or the other.
        #[serde(default, skip_serializing_if = "no_time")]
        time: f32,
        /// Echo spacing in **beats** of the song it plays in — `0.75` is a
        /// dotted eighth — in place of `time`.
        ///
        /// Read off the clock the song is actually rendered at, so it is
        /// resolved **after** a `stretch` fit or an anchor has moved the tempo
        /// and follows a tempo map as it goes: the echoes stay on the grid
        /// however the piece was made to land. A track's or the song's chain
        /// only — a patch has no tempo, and refuses one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        beats: Option<f32>,
        /// How much of each echo feeds the next, `0..1`.
        feedback: f32,
        /// Wet/dry blend, `0..=1`.
        mix: f32,
        /// Whether the repeats alternate sides: the first on the left, the
        /// next on the right, and so on down the tail.
        ///
        /// Absent — and a delay is a line per channel, which on a centred
        /// source is two identical echoes and therefore a position in
        /// **time** and never in width. Present, it is the one recognisable
        /// stereo delay, and the only thing in this crate besides `chorus`
        /// and the `reverb` that puts a sound somewhere the `pan` did not.
        ///
        /// One flag rather than a second `time`: an independent delay per
        /// side expresses more — a few milliseconds' offset is a Haas spread,
        /// which is a different effect entirely — and is correspondingly
        /// easier to write wrong, and a recipe that wants two delays in two
        /// places can already write two of them and pan the tracks.
        #[serde(default, skip_serializing_if = "not_ping_pong")]
        ping_pong: bool,
    },
    /// Freeverb room reverb: eight combs and four allpasses.
    Reverb {
        /// Room size, `0..=1`.
        size: f32,
        /// High-frequency damping, `0..=1` — how quickly the tail dulls.
        damp: f32,
        /// Wet/dry blend, `0..=1`.
        mix: f32,
        /// Seconds of silence between a sound and the first of its room, up to
        /// half a second. Absent is none, which is Freeverb as it always was:
        /// the tail starting on the transient and smearing it. `0.02`–`0.06`
        /// keeps a snare's crack or a lead's attack clear of the room it is
        /// in — the gap a big hall has before its first wall answers.
        #[serde(default, skip_serializing_if = "no_predelay")]
        predelay: f32,
    },
    /// A `tanh` soft clip: the one nonlinearity in the signal path, and so the
    /// only stage that can put a harmonic into the output that was not in the
    /// input. What "warm" and "glued" are made of.
    Saturate {
        /// How hard the signal is pushed into the curve. `0` is the identity
        /// line, `1`–`2` is warmth, `4` is audible drive; clamped at a ceiling
        /// past which a soft clip is a fuzz pedal. It is gain-compensated, so
        /// this changes the shape of the wave and not its peak.
        drive: f32,
        /// Wet/dry blend, `0..=1`. Parallel drive — a hard-driven copy under a
        /// clean one — is how weight is added without rounding the transients
        /// off, and it costs nothing over a full-wet setting.
        mix: f32,
    },
    /// A compressor: the level control that acts on the loud moments and
    /// leaves the quiet ones, which is what makes a part *sit* rather than
    /// merely be at a volume.
    ///
    /// A limiter is not this at another setting: one is chosen and meant to be
    /// heard, the other is unconditional and is the promise that a bake cannot
    /// clip. The `fx::compress` module carries the whole argument.
    Compress {
        /// The level above which the signal is pushed down, in dBFS. Clamped
        /// to −60…0: full scale is where the limiter's job starts.
        threshold: f32,
        /// How much of each decibel over the threshold survives, as `n:1` —
        /// `2` is gentle, `4` is the workhorse, `10` is a part pinned in
        /// place. `1` is no compression at all, and is an exact bypass unless
        /// `makeup` is asking for something. Clamped at 20:1, past which this
        /// would be a limiter.
        ratio: f32,
        /// Seconds the gain takes to arrive at full reduction. Because this
        /// renders offline the duck is *ready* when the peak lands rather than
        /// chasing it, which is a more transparent attack than a hardware
        /// compressor's and lets a transient through less; `mix` is how a
        /// recipe keeps one.
        #[serde(default = "attack_default")]
        attack: f32,
        /// Seconds the gain takes to recover afterwards. Too short and the
        /// mix breathes audibly between hits; a tenth of a second upwards is
        /// the usual range.
        #[serde(default = "release_default")]
        release: f32,
        /// Decibels handed back to the compressed copy, to replace what the
        /// reduction took. Clamped to ±24.
        #[serde(default)]
        makeup: f32,
        /// Wet/dry blend, `0..=1`. Below 1 this is **parallel** ("New York")
        /// compression: a squashed copy under an untouched one, which adds
        /// density without flattening the transients. `0.0` is an exact
        /// bypass.
        #[serde(default = "one")]
        mix: f32,
        /// Another track, by name, whose part the detector listens to instead
        /// of this one's — the kick pressing the bass down on every beat.
        ///
        /// **A track chain only.** A patch chain runs per note and the song's
        /// runs on the sum; neither sits anywhere a track name can be read, so
        /// both refuse this rather than ignoring it. The part handed over is
        /// the key track's *as played* — before its own chain, its `gain` and
        /// its `pan` — so `crate::song`'s mixer has the reasoning and the
        /// consequence: two tracks may key each other, because neither is
        /// waiting on the other's output.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sidechain: Option<String>,
    },
    /// Several detuned, delayed copies of the signal, spread across the
    /// stereo field: the difference between one synthesiser playing a note and
    /// a section playing it. The only effect here that makes one source sound
    /// like more than one thing.
    Chorus {
        /// How fast each copy's delay sweeps, in Hz. `0.3`–`1.5` is the usual
        /// range; the voices are spread slightly either side of it so the
        /// ensemble does not breathe in unison. Clamped at 10, past which the
        /// modulation sidebands stop being a detune.
        rate: f32,
        /// How far the sweep moves, `0..=1` — how far apart the copies are
        /// pushed in pitch. Around `0.3` is a shimmer, `0.8` is a wide
        /// ensemble, and `1.0` starts to sound seasick on a sustained note.
        depth: f32,
        /// How many copies, clamped to 2–4. Two is a double-tracked part, four
        /// is a section; past four each new voice sits on top of one already
        /// there. It is a thickness control and not a fader — the copies are
        /// normalised, so the wet signal is the same level whatever this says.
        #[serde(default = "voices_default")]
        voices: usize,
        /// Wet/dry blend, `0..=1`. The dry signal stays exactly where it was
        /// and the copies arrive around it, so this is how far the source
        /// spreads rather than how loud the effect is.
        mix: f32,
    },
    /// A small stack of filter bands: the treatment for what the bake report
    /// diagnoses. A gain fader answers a muddy pad by removing the pad; this
    /// answers it by removing 250 Hz and keeping the rest.
    Eq {
        /// The bands, applied in list order, at most [`MAX_EQ_BANDS`] of them.
        /// Order between them barely matters — filters commute to within
        /// rounding — but a list is a list, and it is honoured as written.
        bands: Vec<EqBand>,
    },
}

/// Whether a delay's repeats stay on the side they arrived on — the test that
/// keeps `"ping_pong": false` out of every saved document.
///
/// A field that is absent when it does nothing matters more here than
/// elsewhere: a bake is addressed by the hash of the recipe's bytes, alongside
/// [`SYNTH_VERSION`](crate::SYNTH_VERSION), so a serialiser that started
/// writing a default into every chain would invalidate every cached bake in
/// every project at once, for no change in the audio — which is the cost this
/// crate pays deliberately when the audio *does* change, and never otherwise.
/// `song`'s own `no_swing` is the same rule at the other document.
fn not_ping_pong(ping_pong: &bool) -> bool {
    !*ping_pong
}

/// Whether a delay leaves its spacing to `beats` — the test that keeps
/// `"time": 0.0` out of a beat-timed delay, for the reason [`not_ping_pong`]
/// gives. A delay written with `"time": 0` was a no-op, and still is.
fn no_time(time: &f32) -> bool {
    *time == 0.0
}

/// Whether a room answers at once — the test that keeps `"predelay": 0.0`
/// out of every saved document, for the reason [`not_ping_pong`] gives.
fn no_predelay(predelay: &f32) -> bool {
    *predelay == 0.0
}

fn gentle_q() -> f32 {
    0.707
}

/// A compressor's attack when the recipe does not say: 10 ms, fast enough to
/// catch a hit and slow enough not to be shaping the waveform.
fn attack_default() -> f32 {
    0.01
}

/// A compressor's release when the recipe does not say: 150 ms, roughly one
/// beat at a mid tempo, which is where a sidechain duck sounds like the track
/// rather than like an effect.
fn release_default() -> f32 {
    0.15
}

fn voices_default() -> usize {
    3
}
