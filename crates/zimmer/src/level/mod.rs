//! How a finished sound came out: over its whole length, over time, and across
//! the spectrum.
//!
//! Pure arithmetic over samples, which is why it lives here rather than beside
//! either of the two things that want it. A render measures its finished mix
//! and a bake measures the samples it just synthesised; those are different
//! crates with different jobs, and the measurement is the same measurement.
//! Putting it in the one crate that is *only* sample arithmetic is what keeps
//! there from being two of it.
//!
//! **This is a signal, never a gate.** There is no correct loudness — a sting
//! is meant to be hot, a bed under narration is meant to be far down — so a
//! threshold that refused a bake or a render would be a taste enforced as a
//! build failure. What it changes is the default: from "sounds fine, probably"
//! to a number the author can act on.
//!
//! The defects that argued for it were all **authored content and not code**: a
//! noise swell loud enough to wash out a whole piece, and two scores that came
//! out four and a half decibels under the reference they replaced. No test
//! suite catches those, because they are artistic choices with wrong numbers
//! rather than regressions.
//!
//! ## Why one number for a whole file is not enough
//!
//! **A mean hides everything that changes.** A song whose middle section is
//! buried and whose ending is twice as loud as its opening reports one
//! unremarkable average. Every problem worth finding in a piece of music is a
//! problem *at a particular moment*, and a whole-file statistic is exactly the
//! wrong shape for it — the noise swell above was a change over time, reported
//! as a constant. [`Profile`] is the answer to that: the same statistics again,
//! a section at a time.
//!
//! **Level is not the only way a mix is wrong.** A piece can measure perfectly
//! and still be mud, because everything is stacked in the same octave and the
//! low end is most of the energy. [`Bands`] catches "muddy" and "thin", which a
//! meter cannot see at all.
//!
//! **A number about the whole says nothing about who caused it.** "87% of the
//! energy is below 250 Hz" is a correct diagnosis of a five-instrument mix and
//! an address nobody can act on: the fix is to change four of them at once and
//! re-measure. [`Layer`] is the same statistics a third way — one row per part
//! of the mix — which is what turns adjusting into measuring.
//!
//! **A difference is easier to judge than a number.** Is −14 dBFS good? It
//! depends entirely. Is it 4.6 dB under the version it was meant to replace?
//! That is a finding. [`Difference`] is the form an agent iterating on a score
//! actually needs, and the form in which two of the three defects above were
//! ever visible.
//!
//! **Some defects live between samples.** A click, a 10 ms dropout and a fade
//! that arrives as a staircase of gain steps all average away inside any
//! window long enough to measure loudness over, and they are what a listener
//! calls "choppy". [`DefectFinder`] reads them a moment at a time, each with
//! its time, channel and size.
//!
//! ## The one piece of this that is not a report
//!
//! The `intersample` module reconstructs the waveform *between* two samples,
//! and it is the only thing here read from outside: the master limiter holds a
//! bake under a ceiling measured exactly the way [`Loudness::true_peak_dbfs`]
//! reports it, and [`FramePeaks`] hands the same per-frame reading to
//! `scorsese-render`, whose delivery limiter holds a mix brought up to a
//! loudness target under the same ceiling (#968). That sharing is deliberate. A guarantee and a measurement that
//! disagree about where a waveform goes between its samples is how a bake came
//! to be limited and reported as clipping at the same time, and neither file
//! could have caught it alone.
//!
//! ## What this is deliberately not
//!
//! **Not a critic, and not a substitute for ears.** Measurement finds
//! *defects* — too quiet, clipping, muddy, a section flat where the arrangement
//! said climax. It does not find *taste*: no statistic here will notice that a
//! melody is charmless, and a metric treated as an ear produces music that
//! optimises the number and gets worse. The honest claim is smaller and still
//! valuable — when the author says "the bass is weak in the middle", the change
//! can be verified without asking the author to listen a second time.
//!
//! **Not a musical analysis.** No key detection, no chord detection, no
//! self-similarity. For a `synth_audio` asset the recipe already states all of
//! that, exactly and for free; recovering it from the audio would be spending
//! effort to produce a worse copy of a document already in hand. The territory
//! here is only what exists **after** the render — level, spectral balance, and
//! whether section C is actually bigger than section A or merely has more notes
//! in it.

pub(crate) mod bands;
pub(crate) mod defects;
pub(crate) mod diff;
pub(crate) mod gain;
pub(crate) mod integrated;
pub(crate) mod intersample;
pub(crate) mod layer;
pub(crate) mod meter;
pub(crate) mod peaks;
pub(crate) mod profile;

pub use bands::{BandMeter, Bands};
pub use defects::{Defect, DefectFinder, Defects, Tally, WORST};
pub use diff::{BandsDifference, Difference};
pub use gain::{db_to_gain, db_to_gain_f32, gain_to_db, gain_to_db_f32};
pub use integrated::Integrated;
pub use layer::Layer;
pub use meter::{Loudness, Meter};
pub use peaks::FramePeaks;
pub use profile::{Cut, Profile, Profiler, Span};
