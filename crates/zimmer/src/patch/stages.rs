//! The stages a patch is built from: what makes the tone, and what shapes it.
//!
//! What is done to it *afterwards* is the chain, and that lives in
//! [`fx`](super::fx) — one file per question, because the chain is the one
//! part of a patch that is also written in two other places (a track's, and
//! the song's) and is read there without any of this.
//!
//! Every type here is plain serde data — no buffers, no handles — so a patch
//! round-trips losslessly through JSON and means the same thing whether it was
//! written by hand or by an agent.

use serde::{Deserialize, Serialize};

use super::fm::{Algorithm, FM_OPERATORS, Operator};

/// The most oscillators one stack may carry. Four detuned saws is already a
/// fat analog lead; past that it is CPU for no audible gain.
pub const MAX_OSCS: usize = 4;

/// The most detuned copies one oscillator may sound at once.
///
/// Seven, which is the supersaw's own number and not a round one: the voice
/// count the JP-8000 shipped with is what the sound is, and every unison patch
/// written since has been reaching for it. Past seven the copies land closer
/// together than the ear separates them, and the arithmetic is the whole
/// oscillator again — the same argument [`MAX_OSCS`] makes, applied one level
/// down, where a stack of four at seven voices is already twenty-eight
/// oscillators for one note.
pub const MAX_VOICES: usize = 7;

/// The most partials one additive series may carry.
///
/// Sixteen, and the number is argued the way [`MAX_OSCS`] is rather than
/// copied from it. Sixteen partials cover the whole audible harmonic series of
/// any note up to about 1.3 kHz; above that the top of the series is over
/// 22 kHz and is dropped at Nyquist regardless of this cap. Below it, the
/// seventeenth harmonic of any timbre this crate is for sits far enough down
/// that nothing is missed by its absence — a drawbar organ offers nine, and
/// the ear stops separating partials well before it stops hearing them.
///
/// Past the cap it is one more sine oscillator per note for no audible gain,
/// and an uncapped list is an unbounded allocation, which is one of the few
/// things this crate refuses outright.
pub const MAX_PARTIALS: usize = 16;

/// One oscillator's waveform.
///
/// `saw` and `square` are band-limited (polyBLEP) at render time — the naive
/// versions alias audibly at high pitch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wave {
    /// A pure tone: one harmonic, nothing for a filter to work on.
    Sine,
    /// Odd harmonics falling away steeply. Soft, flute-ish.
    Triangle,
    /// Every harmonic. The classic subtractive starting point.
    Saw,
    /// Odd harmonics only — hollow, and the head of most chiptune leads.
    Square,
}

/// How a noise source's energy is spread across the spectrum.
///
/// Named for the slope rather than the use, because one colour is many
/// sounds: pink is wind, surf, rain and room tone as well as the body of a
/// snare. What each one *is* — the filter behind it, the measured slope, and
/// why a lowpass over white is not a substitute for either — is
/// `core::noise::color`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseColor {
    /// Equal energy per hertz, so most of it is in the top octave: a hiss.
    /// Cymbals, static, air.
    #[default]
    White,
    /// −3 dB per octave — equal energy per octave, the balance most natural
    /// sound falls in. Wind, surf, rain, room tone, and a far better snare or
    /// cymbal body than white under a filter.
    Pink,
    /// −6 dB per octave, an integrated draw. Thunder, rumble, distant
    /// traffic, and the low half of an impact.
    Brown,
}

/// One oscillator in a stack: its wave, its detune from the played pitch, its
/// weight in the mix, a whole-octave transpose, and how many detuned copies of
/// itself it sounds at once.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Osc {
    /// Which waveform this oscillator produces.
    pub wave: Wave,
    /// Detune in cents (100 cents = a semitone). Small opposing detunes across
    /// a stack are what make a "fat" lead.
    #[serde(default)]
    pub detune_cents: f32,
    /// Weight in the stack mix; the stack is normalised by the sum of gains.
    #[serde(default = "one")]
    pub gain: f32,
    /// Whole-octave transpose, e.g. `-1` for a sub-oscillator.
    #[serde(default)]
    pub octave: i32,
    /// How many detuned copies of this oscillator sound at once — unison.
    ///
    /// `1`, the default, is one oscillator and is exactly what every patch
    /// written before this field existed already meant. Past that the entry
    /// becomes `voices` copies spaced evenly across [`Osc::spread`], each with
    /// its own start phase, which is the supersaw.
    ///
    /// **Unison lives here rather than in the stack** because it is one timbre
    /// and not several. Written out as separate oscillators it costs the whole
    /// [`MAX_OSCS`] budget on a single sound and leaves nothing for the
    /// sub-oscillator or the second wave that a patch actually wants beside
    /// it, and every one of those entries repeats the same wave, gain and
    /// octave to say one thing.
    ///
    /// **It is a thickness control and not a fader.** The copies are
    /// normalised by their own count, so `voices: 7` is the same weight in the
    /// stack that `voices: 1` was; without that, adding unison to one
    /// oscillator would move every gain in the song.
    ///
    /// At most [`MAX_VOICES`], and at least one — a stack refuses both ends
    /// rather than quietly rendering something the recipe did not write.
    #[serde(default = "solo", skip_serializing_if = "is_solo")]
    pub voices: usize,
    /// How far apart the outermost unison voices sit, in cents — the full
    /// width of the spread, not the distance from the centre.
    ///
    /// Read only when [`Osc::voices`] is above one, and centred on
    /// [`Osc::detune_cents`], so a spread never moves the pitch the
    /// oscillator was written at. The default is a shimmer; `25` is a fat
    /// lead and past `50` the voices start to be heard as a chord rather than
    /// as one thick note.
    #[serde(default = "spread_default", skip_serializing_if = "is_default_spread")]
    pub spread: f32,
    /// How far across the stereo field the unison voices fan out, `0..=1`.
    ///
    /// Read only when [`Osc::voices`] is above one. The voices are placed
    /// evenly from `-width` to `+width` on the pan axis, lowest-pitched on the
    /// left, so `1` puts the outermost two hard left and hard right and the
    /// middle one, at an odd count, dead centre. Each copy keeps its own
    /// pitch and start phase, so the two sides carry different signals: that
    /// is what a wide pad or a supersaw lead *is*, and what a chorus can only
    /// imitate by smearing one signal against a delayed copy of itself.
    ///
    /// `0`, the default, keeps every voice centred, and is exactly what every
    /// patch written before this field existed already rendered. Outside
    /// `0..=1` it is clamped rather than refused, the way a track's `pan` is:
    /// a value past the edge is a taste, not a fault.
    #[serde(default, skip_serializing_if = "is_centred")]
    pub width: f32,
    /// How much of each cycle a `square` spends high, `0.05..=0.95` — its
    /// duty cycle.
    ///
    /// `0.5`, the default, is the square every patch written before this
    /// field rendered. `0.25` and `0.125` are the other two chiptune duties,
    /// each a distinct instrument; a narrow pulse is the nasal reed and clav
    /// tone, and one an [`LfoTarget::PulseWidth`](super::LfoTarget::PulseWidth) keeps moving is the warm
    /// analogue pad. A narrower pulse is quieter and thinner: that is the
    /// sound, so no level is made up for it. Past the range it is clamped,
    /// since a pulse of zero or full width is silence. On any other wave it
    /// is refused, a field nothing could hear; every unison voice shares it.
    #[serde(default = "half", skip_serializing_if = "is_half")]
    pub pulse_width: f32,
}

/// The narrowest pulse a square is drawn at; [`Osc::pulse_width`] below it
/// is clamped up to it.
pub const MIN_PULSE_WIDTH: f32 = 0.05;

/// The widest pulse a square is drawn at, the mirror of [`MIN_PULSE_WIDTH`].
pub const MAX_PULSE_WIDTH: f32 = 0.95;

/// One partial of an additive series: where it sits, how much of it there is,
/// how far off the harmonic grid it is bent, and how fast it dies on its own.
///
/// A drawbar, in other words — and that is the whole idea of the source. Every
/// other head of the signal path generates something rich and lets a filter
/// carve it down; a table of these *states* the spectrum, so the timbre is the
/// numbers rather than what a filter shape happened to arrive at.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partial {
    /// Frequency as a multiple of the played pitch: `1` is the fundamental,
    /// `2` the octave above it, `3` the twelfth above that.
    ///
    /// Whole numbers keep the series harmonic, which is what a tone with a
    /// definite pitch is made of. Fractional ones are legal and go inharmonic
    /// — a bell, a struck plate, a glass — and unlike the fractional ratio an
    /// [`Source::Fm2`] modulator takes, each partial is *placed* rather than
    /// arriving as a sideband of one number chosen for its overall effect.
    ///
    /// A ratio at or below zero is refused: it names no frequency, and zero in
    /// particular is a DC offset rather than a sound.
    pub ratio: f32,
    /// Weight in the mix. The series is normalised by the sum of the gains of
    /// the partials that actually sound, so adding one thickens the tone
    /// without making it louder.
    #[serde(default = "one")]
    pub gain: f32,
    /// Detune off `ratio`, in cents (100 cents = a semitone).
    ///
    /// The route to *chosen* inharmonicity. Real strings are slightly
    /// stretched — a piano's upper partials sit progressively sharp of the
    /// whole numbers, and that stretch is a good part of why a piano sounds
    /// like a piano while a perfectly harmonic stack sounds like an organ. A
    /// few cents more on each partial than the last is exactly that, written
    /// down.
    ///
    /// Cents rather than Hz for the reason [`PitchEnv::semitones`] gives:
    /// pitch is logarithmic, so a detune in Hz would mean a different interval
    /// at every pitch the patch is played at.
    #[serde(default)]
    pub detune_cents: f32,
    /// Seconds for this partial's **own** exponential decay, underneath the
    /// patch's amp envelope. `0` — the default — is a partial that does not
    /// fade by itself.
    ///
    /// This is the field the source is really for. On anything bowed, blown or
    /// struck the upper partials die sooner than the fundamental, and a tone
    /// whose partials all fade together is the one thing no filter sweep quite
    /// fakes. Give the fundamental a long decay and the sixth partial a short
    /// one and the note is bright at its attack and settles into its body,
    /// which is what a real one does.
    ///
    /// The two envelopes **multiply**, and the shorter always wins. The
    /// renderer's own module doc states that relationship in full; the short
    /// version is that a patch letting the partials do the shaping wants an
    /// amp envelope that sustains, or the note dies twice.
    #[serde(default)]
    pub decay: f32,
}

/// What generates the raw tone. Exactly one per patch — the head of the fixed
/// signal path.
///
/// Six of them, in three groups, and they are listed in that order because it
/// is also how a recipe chooses:
///
/// - **Make something rich and carve it.**
///   [`OscStack`](Source::OscStack) generates every harmonic,
///   [`Karplus`](Source::Karplus) excites a string and lets it ring, and
///   [`Noise`](Source::Noise) is every frequency at once, in whichever balance
///   its [`color`](NoiseColor) names. What the patch says is mostly what to
///   take *away*, downstream at the filter.
/// - **Bend one sine with another.** [`Fm2`](Source::Fm2) is one modulator on
///   one carrier; [`Fm4`](Source::Fm4) is four operators wired by a chosen
///   algorithm. What the patch says is a *depth*, and the spectrum is whatever
///   falls out of it. They are two sizes of one idea and both are kept — a
///   recipe that has outgrown the smaller one knows exactly what it wants next.
/// - **State the spectrum outright.** [`Additive`](Source::Additive) is a
///   table of partials, each placed, weighted and faded on its own. What the
///   patch says *is* the answer, with nothing left for a filter to arrive at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    /// Subtractive stack: up to [`MAX_OSCS`] oscillators summed, the classic
    /// analog-synth head.
    OscStack {
        /// The oscillators, summed and normalised by the sum of their gains.
        oscs: Vec<Osc>,
    },
    /// Karplus-Strong plucked string: a noise-excited delay line with damped
    /// feedback. Guitars, basses, harps, marimbas.
    Karplus {
        /// Feedback scale per round trip, `0..1` — how long the string rings.
        #[serde(default = "damping_default")]
        damping: f32,
        /// Excitation brightness, `0..1`: 0 is a soft, pre-lowpassed pluck, 1
        /// is full white noise (a hard pick).
        #[serde(default = "half")]
        brightness: f32,
    },
    /// Raw noise — the head of every gunshot, impact and footstep, shaped by
    /// the filter and amp envelope.
    Noise {
        /// How its energy is spread across the spectrum. Absent means
        /// [`NoiseColor::White`], the hiss this source has always been.
        #[serde(default, skip_serializing_if = "is_white")]
        color: NoiseColor,
    },
    /// Two-operator FM: a sine modulator at `ratio × f` bending a sine
    /// carrier. Electric pianos, bells, metallic hits.
    Fm2 {
        /// Modulator frequency as a multiple of the played pitch. Integer
        /// ratios are harmonic (tonal); non-integer ratios go inharmonic
        /// (metallic).
        ratio: f32,
        /// Modulation depth. Higher indices add sidebands, i.e. brightness.
        index: f32,
        /// How much modulation depth a full-velocity strike adds to `index`.
        /// Defaults to `0.0`: brightness fixed however hard the note is
        /// struck, which is what every patch written before this field
        /// existed already meant.
        ///
        /// The FM half of the velocity routing described on
        /// [`Filter::vel_octaves`], and the more literal half: in two-operator
        /// FM the index *is* the brightness, so this is the whole difference
        /// between a bell tapped and a bell hit.
        ///
        /// Added to `index` and then floored at zero. The floor is not
        /// defensive — a negative index only mirrors the modulator, and a
        /// mirrored modulator sounds exactly as bright as its positive twin,
        /// so an unclamped sum would make a darkening routing start
        /// brightening again the moment it crossed over.
        #[serde(default)]
        vel_index: f32,
        /// Seconds for the modulator's own decay — how fast the bright attack
        /// collapses to a near-sine body.
        #[serde(default = "mod_decay_default")]
        mod_decay: f32,
    },
    /// Four-operator FM: four sines wired by a chosen algorithm. Brass,
    /// basses, sustained pads, and bell-into-body layers — the FM territory
    /// two operators cannot reach, because two of them can only produce one
    /// modulator-carrier relationship.
    ///
    /// [`Source::Fm2`] is not superseded and is usually the right one: it is
    /// two numbers rather than a routing and four operators, and it covers
    /// bells, electric pianos and struck metal well. Reach for this when the
    /// sound needs a modulator with its *own* modulator, or two voices layered
    /// into one note — see [`Algorithm`] for the routings and [`Operator`] for
    /// what each of the four carries.
    Fm4 {
        /// How the four are wired. There is no default: the routing is the
        /// parameter that decides what kind of sound this is, and a silent one
        /// would make eight very different instruments look like one.
        algorithm: Algorithm,
        /// The four operators, in the order the algorithm diagrams number them
        /// — exactly [`FM_OPERATORS`] of them, so a list of three is refused as
        /// it is parsed rather than rendered with a stand-in.
        operators: [Operator; FM_OPERATORS],
        /// How much modulation depth a full-velocity strike adds to **every
        /// operator the algorithm uses as a modulator**.
        ///
        /// The [`Source::Fm2`] field of the same name, applied to a routing:
        /// in FM the index *is* the brightness, so this is the difference
        /// between a horn played softly and one leaned on. Carriers are left
        /// alone — a carrier's level is its share of the mix, and velocity
        /// already reaches the level through the amp envelope.
        ///
        /// Defaults to `0.0`. Added to each modulator's level and then floored
        /// at zero, for the reason [`Source::Fm2`]'s `vel_index` is: a
        /// negative index only mirrors the modulator and sounds exactly as
        /// bright as its positive twin, so an unclamped sum would make a
        /// darkening routing start brightening again the moment it crossed
        /// over.
        #[serde(default)]
        vel_index: f32,
    },
    /// An additive series: up to [`MAX_PARTIALS`] sine partials, each placed,
    /// weighted and faded on its own, summed into one tone. Organs, bowed and
    /// blown sustains, glass, and bells that are not FM-metallic.
    ///
    /// The one source here that states a spectrum instead of shaping one.
    Additive {
        /// The partials, summed and normalised by the sum of the gains of the
        /// ones that sound. Order carries no meaning — a series is a set of
        /// frequencies, not a chain — but it is what decides which partial a
        /// refusal is counted against.
        partials: Vec<Partial>,
    },
}

impl Source {
    /// What this generator is called, in the word the document spells it with.
    ///
    /// The serde tag rather than a prose label, so a report that names a source
    /// and a recipe that chooses one use the same vocabulary — a reader who
    /// sees `karplus` in a report can search the recipes for it.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OscStack { .. } => "osc_stack",
            Self::Karplus { .. } => "karplus",
            Self::Noise { .. } => "noise",
            Self::Fm2 { .. } => "fm2",
            Self::Fm4 { .. } => "fm4",
            Self::Additive { .. } => "additive",
        }
    }
}

/// An ADSR envelope: three timed segments and a level held between them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adsr {
    /// Attack: seconds from silence to full level.
    pub a: f32,
    /// Decay: seconds from full level down to `s`.
    pub d: f32,
    /// Sustain: the level held while the note is, in `0..=1`.
    pub s: f32,
    /// Release: seconds from the sustain level back to silence once the gate
    /// closes. This is why a rendered note is longer than its duration.
    pub r: f32,
    /// How the three timed segments bend on their way to their destination.
    /// `0.0` is the straight line every envelope was before this field
    /// existed; positive is the exponential approach a physical thing makes.
    ///
    /// Nothing physical decays in a straight line. A plucked string, a struck
    /// bell and a decaying room all lose energy in proportion to how much they
    /// still have, which is an exponential, and the ear is unusually good at
    /// telling the two apart: a linear decay holds up too long and then
    /// arrives at silence too abruptly, and that *sag* is one of the handful
    /// of cues that reliably says "synthesised" about an otherwise well-made
    /// sound.
    ///
    /// One number for the whole envelope rather than one per segment, because
    /// the same curve is the right one in all three places — attack, decay and
    /// release are each an approach to a destination, and an exponential
    /// approach means *fast at first, easing in*. On the way up that is a
    /// capacitor charging; on the way down it is the same capacitor
    /// discharging.
    ///
    /// Negative is legal and inverts the bend into a slow start and a sudden
    /// arrival — not a shape anything physical makes, but a deliberate and
    /// useful one for a swell. Magnitudes past what the renderer can evaluate
    /// without overflowing are clamped; the `env` module names the number.
    ///
    /// **Defaulting to linear is load-bearing, not timidity.** A bake is
    /// addressed by the hash of its recipe's bytes, so an envelope that
    /// started curving on its own — or a serialiser that started writing
    /// `"curve": 0.0` into every document — would invalidate every cached bake
    /// in every project at once. Changing this default later is therefore a
    /// deliberate one-time break, argued in its own pull request, never a
    /// quiet tweak.
    #[serde(default, skip_serializing_if = "is_linear")]
    pub curve: f32,
}

/// Whether an envelope bends at all — the test that keeps `"curve": 0.0` out
/// of every saved document, for the reason [`Adsr::curve`] gives.
fn is_linear(curve: &f32) -> bool {
    *curve == 0.0
}

impl Default for Adsr {
    /// A quick, fully-sustaining envelope — an organ-ish "just pass it
    /// through".
    fn default() -> Self {
        Self {
            a: 0.005,
            d: 0.0,
            s: 1.0,
            r: 0.05,
            curve: 0.0,
        }
    }
}

/// An envelope aimed at the note's own pitch: a sweep that happens once and
/// settles.
///
/// This is the primitive an [`Lfo`](super::Lfo) aimed at [`LfoTarget::Pitch`](super::LfoTarget::Pitch) is not. A
/// vibrato is cyclic — it wobbles around the played note for as long as the
/// note lasts. A great many instruments instead start *off* their pitch and
/// arrive on it exactly once: a kick drum starts near 90 Hz and is at 50 in
/// about 40 ms, and a tom, an 808, a timpani and a laser zap are the same move
/// at different speeds and depths. Without this the only way to write one is
/// to fake it through the filter, which approximates the brightness of the
/// gesture and none of the pitch.
///
/// It stacks additively with a vibrato rather than replacing it, the way
/// [`Filter::env_octaves`] and [`Filter::vel_octaves`] do: a patch may sweep
/// down onto its note and then wobble around it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PitchEnv {
    /// How many semitones the envelope adds at full level.
    ///
    /// The played note is where the sweep **ends**, not where it starts: under
    /// the usual shape — full immediately, decaying to nothing — the note
    /// begins `semitones` away and arrives on the pitch it was played at. So
    /// **positive falls onto the note from above**, which is what a kick, a
    /// tom and an 808 do, and negative rises onto it from below, which is a
    /// reverse zap. Both directions are one sign apart and neither is
    /// privileged; naming the destination is what makes a drum patch transpose
    /// like an instrument.
    ///
    /// Semitones rather than Hz because pitch is logarithmic: the same number
    /// of Hz is an octave low down and a rounding error high up, so a sweep
    /// written in Hz stops meaning the same gesture the moment the patch is
    /// played at another pitch. In semitones it transposes with the note.
    pub semitones: f32,
    /// The envelope doing the sweeping, with its own timings and curve.
    ///
    /// Defaults to [`Adsr::default`], which sustains — so a sweep worth
    /// hearing spells out a decay and a sustain of zero, the shape that says
    /// "move once, then settle".
    #[serde(default)]
    pub adsr: Adsr,
}

/// Which side of the filter is kept.
///
/// **The spelling rule for every filter name in this crate lives here**, and
/// it is deliberate rather than an oversight. `lowpass`, `highpass`,
/// `bandpass`, `notch`, and — over in [`EqKind`](super::fx::EqKind) — `lowshelf`, `highshelf`
/// and `peak` are written as **one word with no underscore**, which is the
/// odd one out in a format that otherwise spells `detune_cents`, `gain_db`
/// and `env_octaves` in snake_case.
///
/// The reason is not tradition. In prose the tradition writes *low-pass*, and
/// both `lowpass` and `low_pass` render that into an identifier equally
/// faithfully, so the tradition does not reach as far as the underscore. The
/// reason is **reflex and precedent**: `lowpass` is what somebody arriving
/// from any synthesiser types without thinking about it, and the Web Audio
/// API — the most widely used audio API there is — spells all of these
/// exactly this way. That is what a reader and a model already carry in
/// memory, and a vocabulary that has to be looked up costs a round trip every
/// single time it is written.
///
/// So these are terms imported from a domain that already spells them, and
/// the inconsistency with the rest of the format is the accepted price. It
/// buys one spelling of `low`/`high` + `pass` across the whole crate, which is
/// the thing that was actually going wrong: the two surfaces used to disagree,
/// and both parsers are strict, so every disagreement was a refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterKind {
    /// Keep what is below the cutoff: the darkening move.
    Lowpass,
    /// Keep what is above it: thins a sound out, and is how a small speaker
    /// gets simulated.
    Highpass,
    /// Keep a band around the cutoff and drop both ends. A telephone, a radio,
    /// a wah, and the one way to sweep a resonant peak across a sound without
    /// also changing how loud the whole thing is.
    Bandpass,
    /// Drop a band around the cutoff and keep both ends — the mirror of
    /// [`FilterKind::Bandpass`]. Sweeping one through a sustained chord is a
    /// phaser's whole trick, and taking one ringing region out of a noisy
    /// source is the other use.
    Notch,
}

/// How steeply a filter falls away past its cutoff.
///
/// The difference between a tone control and the thing people mean when they
/// say "filter". One pole pair rolls off gently enough that a lowpass still
/// lets a good deal of the top through; two in series is the aggressive,
/// obviously-filtered sweep, and it is the single most-missed control on a
/// subtractive synth that lacks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slope {
    /// One pole pair — 12 dB per octave. The default, because it is what every
    /// patch written before this field existed already had.
    #[default]
    #[serde(rename = "12db")]
    Db12,
    /// Two pole pairs in series — 24 dB per octave.
    ///
    /// **[`Filter::resonance`] is a different control here.** The emphasis is
    /// applied once per pair, so a setting that merely coloured the cutoff at
    /// 12 dB rings at 24 and one that rang self-oscillates. That is the sound
    /// people are after; it is also why a patch moved from one slope to the
    /// other usually wants its resonance backed off rather than kept.
    #[serde(rename = "24db")]
    Db24,
}

impl Slope {
    /// How many pole pairs are run in series.
    pub(crate) fn pole_pairs(self) -> usize {
        match self {
            Self::Db12 => 1,
            Self::Db24 => 2,
        }
    }
}

/// Whether a filter is at the gentler of the two slopes — the test that keeps
/// `"slope": "12db"` out of every saved document, for the reason
/// `no_swing` gives over in `song/mod.rs`: a bake is addressed by the hash
/// of the recipe's bytes, so a serialiser that started writing a default into
/// every patch would invalidate every cached bake in every project at once,
/// for no change in the audio.
///
/// This branch bumps [`SYNTH_VERSION`](crate::SYNTH_VERSION) anyway, so every
/// bake misses its cache on the way through regardless. The discipline is
/// about every *future* save, which is a separate and still-real concern.
fn gentle(slope: &Slope) -> bool {
    matches!(slope, Slope::Db12)
}

/// Whether a filter's cutoff ignores the note — the test that keeps
/// `"key_track": 0.0` out of every saved document, for the reason
/// [`is_white`] gives.
fn untracked(key_track: &f32) -> bool {
    *key_track == 0.0
}

/// The optional filter stage: a Chamberlin state-variable filter whose cutoff
/// is swept by its own envelope — the move that makes a subtractive patch
/// expressive rather than static.
///
/// **Unknown fields are refused**, which is what makes renaming one of these
/// a loud break rather than a quiet one. `env_amount` and `vel_cutoff` were
/// this stage's modulation depths in Hz; they are now [`Filter::env_octaves`]
/// and [`Filter::vel_octaves`] in octaves. Without this, a recipe still
/// naming the old words would parse, take the `0.0` default for the new ones,
/// and render a filter that simply never moves — the sound silently gone and
/// nothing on the page to say so. With it, serde names the offending word and
/// lists the ones that work.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    /// Which side of the cutoff survives.
    pub kind: FilterKind,
    /// Base cutoff in Hz.
    pub cutoff: f32,
    /// How steeply it falls away past the cutoff. Defaults to
    /// [`Slope::Db12`], which is what every patch written before this field
    /// existed already had.
    #[serde(default, skip_serializing_if = "gentle")]
    pub slope: Slope,
    /// Resonance in `0..1`: emphasis at the cutoff. Near 1 the filter
    /// self-rings — sooner at [`Slope::Db24`], where the emphasis is applied
    /// once per pole pair.
    #[serde(default)]
    pub resonance: f32,
    /// How many **octaves** the filter envelope opens the cutoff by at full
    /// level. Negative sweeps downward.
    ///
    /// Octaves rather than Hz for the reason [`PitchEnv::semitones`] gives
    /// about pitch, and it applies to a cutoff just as hard: the ear hears
    /// ratios, so the same number of Hz is a chasm low down and a rounding
    /// error high up. `3200` used to be an enormous sweep on a filter sitting
    /// at 200 Hz and a modest one on a filter sitting at 6800, which meant the
    /// written number could not be judged, carried from one instrument to
    /// another, or left alone while the patch was transposed. `2.0` opens two
    /// octaves wherever it is written, and that is what the ear was going to
    /// hear anyway.
    ///
    /// The field was **renamed** along with its unit rather than quietly
    /// reinterpreted: an old recipe naming `env_amount` now fails to parse,
    /// which is a refusal rather than a wrong sound.
    #[serde(default)]
    pub env_octaves: f32,
    /// How many **octaves** a full-velocity strike opens the cutoff by.
    /// Defaults to `0.0`, which is velocity doing nothing here — exactly what
    /// every patch written before this field existed already meant.
    ///
    /// This is what makes a note read as *played* rather than *turned up*. On
    /// any real instrument, more energy in means more energy in the upper
    /// harmonics: a hard-picked string is not merely a louder string, it is a
    /// brighter one, and the ear reads that change in brightness as effort.
    /// Velocity aimed only at the fader is a large part of why a carefully
    /// written synthesised part still sounds like a machine.
    ///
    /// Same octave unit and same sign convention as [`Filter::env_octaves`],
    /// because it is the same quantity arriving from a different source — one
    /// mental model, and the two are directly comparable when both are set.
    /// The terms are **added**, and so is an [`Lfo`](super::Lfo) aimed at
    /// [`LfoTarget::Cutoff`](super::LfoTarget::Cutoff), which was already written in octaves:
    /// `cutoff × 2^(env_octaves × env + vel_octaves × vel + lfo)`. Adding the
    /// depths rather than multiplying the results is what keeps each source of
    /// movement independent of the others and a zero harmless; multiplying
    /// would let `vel = 0` shut the filter outright, which is a different and
    /// worse instrument. The sum is an exponent rather than an offset, so
    /// three octaves is three octaves from wherever the cutoff happens to be.
    ///
    /// Negative is legal and means velocity *darkens* — a perfectly good
    /// instrument, and the reason this is not validated as positive. The
    /// resulting cutoff is clamped into the filter's stable band per sample,
    /// exactly as a negative `env_octaves`'s already is, so no value here can
    /// produce an unstable filter.
    #[serde(default)]
    pub vel_octaves: f32,
    /// How far the cutoff follows the note being played, in `0..=1`; clamped.
    /// Defaults to `0.0`: a cutoff fixed in Hz whatever the pitch.
    ///
    /// The cutoff moves by `key_track × (midi − 60) / 12` octaves, a term of
    /// the same sum as [`Filter::env_octaves`] and [`Filter::vel_octaves`]: at
    /// `1.0` it moves an octave per octave, so a melody keeps its harmonics
    /// up high and a bass keeps from buzzing down low. It pivots on middle C,
    /// so the written `cutoff` is what C4 gets. It reads the written note
    /// only, never a glide, pitch envelope or vibrato.
    #[serde(default, skip_serializing_if = "untracked")]
    pub key_track: f32,
    /// The cutoff envelope. Defaults to the same fast shape as
    /// [`Adsr::default`].
    #[serde(default)]
    pub adsr: Adsr,
}

pub(super) fn one() -> f32 {
    1.0
}

/// Whether a noise source is the plain hiss — the test that keeps
/// `"color": "white"` out of every saved document, for the reason
/// [`Adsr::curve`] gives about defaults and cached bakes.
fn is_white(color: &NoiseColor) -> bool {
    matches!(color, NoiseColor::White)
}

/// One oscillator, undoubled: what an `Osc` meant before unison existed.
fn solo() -> usize {
    1
}

/// Whether an oscillator is a single voice — the test that keeps
/// `"voices": 1` out of every saved document, for the reason [`is_white`]
/// gives.
fn is_solo(voices: &usize) -> bool {
    *voices == 1
}

/// How far apart unison voices sit when the recipe does not say: 12 cents
/// end to end, an eighth of a semitone, which is a shimmer rather than a
/// chord.
fn spread_default() -> f32 {
    12.0
}

/// Whether a spread is the written-nothing one, so it is not saved either.
fn is_default_spread(spread: &f32) -> bool {
    *spread == spread_default()
}

/// Whether an oscillator's unison is unspread, the `width` default — so a
/// patch that never mentions it is written back without it.
fn is_centred(width: &f32) -> bool {
    *width == 0.0
}

fn half() -> f32 {
    0.5
}

/// Whether a pulse is the square it always was, so it is not saved either.
fn is_half(pulse_width: &f32) -> bool {
    *pulse_width == half()
}

fn damping_default() -> f32 {
    0.996
}

fn mod_decay_default() -> f32 {
    0.3
}
