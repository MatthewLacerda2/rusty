//! core — the note renderer: patch + note → a stereo pair of `f32` buffers.
//!
//! This is the layer's contract made executable: *(pitch, velocity, duration) in →
//! buffer out*. The signal path is fixed, which is the whole point of a structured
//! patch — the recipe picks what fills each stage, never how they connect:
//!
//! ```text
//!   source ─► filter ─► amp envelope ─┐
//!      ▲         ▲            ▲          │
//!      ├──── envelopes ───────┘          ├─► + ─► fx chain
//!      └─────── LFO ──────────┘          │
//!   (up to three layers: the same path, │
//!    each its own source and shapes) ───┘
//! ```
//!
//! **Layers** are that `source → filter → amp` path again, up to
//! [`MAX_LAYERS`](crate::patch::MAX_LAYERS) more times, summed before the one
//! fx chain ([`voice`]). They share the note — pitch, velocity, timbre, glide,
//! gate and the LFOs — and each draws on its own seed. A song's `cutoff` curve
//! moves the top-level filter only.
//!
//! An envelope has three possible destinations — the amplifier always, and
//! optionally the filter cutoff and the note's own pitch. The pitch envelope is
//! the one-shot the LFO is not: a sweep that happens once and settles, which is
//! what a kick drum, a tom, an 808 and a laser zap all are. It writes into the
//! same per-sample frequency track a vibrato does, in semitones, and the two
//! **add** — so a note may fall onto its pitch and then wobble around it.
//!
//! A note's **velocity** taps that path in up to three places, not one. It
//! always scales the amp envelope; a patch may also aim it at the filter cutoff
//! (`vel_octaves`) and at the FM modulator's depth (`vel_index`), both defaulting
//! to zero. That is what separates a note that was *played* from one that was
//! turned up — striking an instrument harder moves energy into the upper
//! harmonics, and a velocity that only reaches a fader is the reason a
//! carefully written part can still sound mechanical.
//!
//! Those last two are shown a velocity of their **own**: the same number,
//! unless a performance moved this strike's tone off its level, which is what
//! [`NoteOpts::timbre`] carries. Tone and loudness are two hands, and a part
//! whose every note brightens exactly as much as it gets louder is a part
//! played with one.
//!
//! Everything is `f32` in `−1..=1` end to end; the only quantisation is the WAV
//! encoder's. The rendered buffer is **longer than the note**: the amp
//! envelope's release rings out after the gate closes, and an fx chain adds its
//! own tail (an echo cut off mid-repeat is a click, not an echo).
//!
//! **Both channels take the whole path**, from the source stage onward. For
//! every source but `noise`, and an oscillator stack whose unison was given a
//! `width`, the two are computed from identical inputs and so come out
//! bit-identical, which is arithmetic done twice for one answer — and
//! that is the price of the path being uniform. The alternative was a flag
//! saying whether the sides had parted company yet, read by every stage
//! downstream, which is a mode; this is an offline renderer and it can afford
//! the multiply instead. What it buys is that the fx chain, where a stereo
//! reverb lives, needs no special case for the one-shot that reached it dry.
//!
//! Module layout: [`osc`] (band-limited oscillator stack), [`karplus`] (plucked
//! string), [`fm`] (FM, two operators or four), [`additive`] (a stated
//! harmonic series), [`noise`] (the one seeded RNG), [`nyquist`] (what any of
//! them may place above the played pitch), [`source`] (which of those runs),
//! [`mod@env`] (ADSR), [`filter`] (state-variable filter), [`tracks`] (the
//! per-sample curves the stages walk: pitch and cutoff), [`lfo`] (the patch's
//! LFOs over one note, and the tremolo), [`sweep`] (a
//! cutoff moved from outside the note), [`voice`] (one path of a note, and
//! what its layers share), [`trim`] (where
//! a baked one-shot's file ends).

pub(crate) mod additive;
pub(crate) mod env;
pub(crate) mod filter;
pub(crate) mod fm;
pub(crate) mod karplus;
pub(crate) mod lfo;
pub(crate) mod noise;
pub(crate) mod nyquist;
pub(crate) mod osc;
pub(crate) mod source;
pub(crate) mod sweep;
pub(crate) mod tracks;
pub(crate) mod trim;
mod voice;

use super::error::SynthError;
use super::fx;
use super::note::NoteOpts;
use super::patch::Patch;
use super::stereo::Stereo;
use lfo::{Lfos, Onset};
use voice::{Struck, Voice, layer_seed};

/// The one rate everything here renders at. 44.1 kHz is CD rate: the full
/// audible band.
///
/// Fixed rather than chosen per call, and deliberately so. A bake is addressed
/// by the hash of the recipe that made it, so it must not vary with what some
/// later render happens to ask for — and `scorsese-render` resamples every
/// audio source on the way into the mix anyway. The reverb's delay lines are
/// tuned against this number; changing it would detune them silently.
pub const SAMPLE_RATE: u32 = 44_100;

/// [`SAMPLE_RATE`] as the float the DSP works in.
pub(crate) const RATE: f32 = SAMPLE_RATE as f32;

/// Longest buffer one note may render to, tail included. A guard, not a musical
/// limit: a typo in `duration` should fail loudly rather than allocate for an
/// hour of audio — so a note past it is refused ([`SynthError::NoteTooLong`]),
/// never cut off at it.
const MAX_SECONDS: f32 = 60.0;

/// Renders one note of `patch` at MIDI pitch `midi` under `opts`, returning
/// the raw stereo pair — **pre-limiter**, so a caller summing several notes
/// can limit the sum instead of each part of it.
///
/// Velocity is clamped once per path, in one place, and then handed to every stage that reads
/// it. Three stages now do — the FM source, the filter and the amp envelope —
/// and if each clamped for itself, one of them eventually would not, and a
/// note would be struck at three subtly different strengths at once.
///
/// It arrives as **two** numbers rather than one, and they part company only
/// by [`NoteOpts::timbre`]: the level the amp envelope multiplies by, and the
/// velocity the brightness routings are shown. A player varies tone and
/// loudness separately — bow pressure against bow speed — and the routings are
/// where that difference already lives. They are clamped in the same place, to
/// the same band, for the reason above; `timbre` of zero collapses them back
/// into the single number every note was struck at before.
pub(crate) fn render_note(patch: &Patch, midi: f32, opts: &NoteOpts) -> Result<Stereo, SynthError> {
    render_swept(patch, midi, opts, None, None)
}

/// [`render_note`] with the filter's base cutoff moved from outside the note:
/// `sweep` is the cutoff in Hz at each sample-frame from the note's onset,
/// and takes the place of the patch's own `cutoff` for the whole note, its
/// release and its ring-out included. A song's `cutoff` curve is the one
/// caller, so a sweep across one held chord is a sweep (#1123).
///
/// `onset` is where the note sounds in a song, which is what lets an LFO be
/// timed in `beats`; a note rendered on its own has none, and its patch is
/// held to [`Patch::validate`], which refuses one.
pub(crate) fn render_swept(
    patch: &Patch,
    midi: f32,
    opts: &NoteOpts,
    sweep: Option<&dyn Fn(usize) -> f32>,
    onset: Option<&Onset<'_>>,
) -> Result<Stereo, SynthError> {
    match onset {
        Some(_) => patch.validate_in_song()?,
        None => patch.validate()?,
    }
    let gate = gate_length(opts.duration)?;
    // The note lasts as long as its longest-ringing layer does.
    let release = patch
        .layers
        .iter()
        .fold(patch.amp.r.max(0.0), |most, layer| most.max(layer.amp.r));
    let n = sample_count(gate, release + fx::tail_seconds(&patch.fx, &fx::NoTempo))?;
    let lfos = Lfos::of(patch.lfos(), opts.seed, onset, n);
    let note = Struck {
        midi,
        opts,
        pulse: osc::pulse::track(&lfos, n),
        lfos,
        gate,
        n,
    };
    // A song's sweep moves the top-level filter, and only that one.
    let base = sweep.map(|sweep| sweep::swept(sweep, n));
    let mut buf = note.voice(&Voice::top(patch), opts.seed, base.as_deref());
    for (index, layer) in patch.layers.iter().enumerate() {
        let part = note.voice(&Voice::layer(layer), layer_seed(opts.seed, index), None);
        buf.add(&part);
    }
    fx::apply_chain(&mut buf, &patch.fx, RATE);
    fx::close_tail(&mut buf, &patch.fx, RATE, &fx::NoTempo);
    Ok(buf)
}

/// Validates the requested note length, rejecting the values that would render
/// nothing at all.
fn gate_length(duration: f32) -> Result<f32, SynthError> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(SynthError::BadDuration { duration });
    }
    Ok(duration)
}

/// Renders one note, limits it and trims it — what a single baked one-shot
/// needs, so it cannot clip its own file and its file ends where the sound
/// does ([`trim`]).
///
/// Kept apart from [`render_note`] because the song mixer wants the unlimited
/// form: limiting every note before summing them would squash each one's
/// dynamics and then squash the sum again.
pub(crate) fn render_limited(
    patch: &Patch,
    midi: f32,
    opts: &NoteOpts,
) -> Result<Stereo, SynthError> {
    let mut buf = render_note(patch, midi, opts)?;
    fx::limiter::apply(&mut buf, RATE);
    trim::to_floor(&mut buf);
    Ok(buf)
}

/// Samples needed for a note held `held` seconds that rings `tail` more, at
/// least one — or the refusal, when that is longer than [`MAX_SECONDS`].
///
/// Refused rather than clamped: a clamp cut a 70 s pad, or a 58 s drone with a
/// 2.4 s reverb, at sixty seconds with a click and no error (#1093). A tail
/// that is not a number is refused the same way, rather than becoming a
/// one-sample note.
fn sample_count(held: f32, tail: f32) -> Result<usize, SynthError> {
    let seconds = held + tail;
    if !(seconds.is_finite() && seconds <= MAX_SECONDS) {
        return Err(SynthError::NoteTooLong {
            at: None,
            held,
            tail,
            limit: MAX_SECONDS,
        });
    }
    Ok(((seconds * RATE).ceil() as usize).max(1))
}
