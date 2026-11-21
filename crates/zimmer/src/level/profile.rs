//! The same statistics again, a section at a time.
//!
//! A whole-file mean is the wrong shape for every problem worth finding in a
//! piece of music, because every one of them is a problem *at a particular
//! moment*: the middle section that is buried, the ending that is twice the
//! opening, the noise swell that grows until it eats the piece. All three read
//! as one unremarkable average.
//!
//! ## Where a section comes from
//!
//! **The arrangement, when there is one.** A bake of a song knows where its
//! patterns start and end, so the rows are the arrangement's own sections. That
//! is what makes a row actionable — "the second chorus is the quiet one" —
//! rather than merely true. [`Cut`] is how a caller hands those over.
//!
//! **A fixed interval otherwise**, which covers a one-shot, an imported file
//! and a rendered mixdown. [`FALLBACK_SECONDS`] is the grid.
//!
//! Either way the sections are decided *before* any samples arrive: a
//! [`Profiler`] is fed a run at a time, exactly as a [`Meter`] is, because a
//! render's mixdown is written segment by segment and never held whole.

use std::collections::VecDeque;

use super::bands::{BandMeter, Bands};
use super::meter::{Loudness, Meter};

/// How long an unlabelled section runs, in seconds.
///
/// Eight seconds is roughly a phrase: long enough that a row is a stretch of
/// music rather than a transient, short enough that a section that changes
/// halfway through still shows up as two different rows. It only ever applies
/// where the document does not say — a song's own arrangement always wins.
pub(crate) const FALLBACK_SECONDS: f64 = 8.0;

/// One boundary in an arrangement: what plays, and when it ends.
///
/// Ends rather than starts, because that is what a running total of pattern
/// lengths produces and because the first section always starts at zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    /// What the document calls this stretch — a pattern's name.
    pub label: String,
    /// Where it ends, in seconds from the start of the piece.
    pub end_seconds: f64,
}

/// One stretch of a signal, measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    /// What the document calls it, when the document said. `None` for a
    /// fixed-interval row and for the whole-file span.
    pub label: Option<String>,
    /// Where it starts, in seconds.
    pub from_seconds: f64,
    /// Where it ends, in seconds.
    pub to_seconds: f64,
    /// How loud it was.
    pub loudness: Loudness,
    /// Where its energy sat. `None` for a silence, which has no balance.
    pub bands: Option<Bands>,
    /// How wide it was: how much of the signal is common to both channels, in
    /// `-1..=1`. `None` for anything that is not two channels, and for a
    /// signal with a silent channel — the same way [`Bands`] is `None` for a
    /// silence.
    ///
    /// On [`Span`] rather than beside it because a span is what every row of
    /// every table here is made of, and being the same fields in the same
    /// order is the only reason those rows can be read against each other.
    /// `1.0` is mono in a stereo container; **negative is the defect**, and
    /// [`super::Meter::correlation`] carries the whole of what it means.
    pub correlation: Option<f64>,
}

impl Span {
    /// How long it runs.
    pub fn seconds(&self) -> f64 {
        (self.to_seconds - self.from_seconds).max(0.0)
    }

    /// Peak minus mean, in decibels — the cheapest proxy there is for "does
    /// this have dynamics, or is it a wall".
    ///
    /// One subtraction and no new machinery. A large crest is a signal with
    /// room in it; a small one is either a deliberately squashed master or a
    /// mix that has been limited into a brick.
    pub fn crest_db(&self) -> Option<f64> {
        Some(self.loudness.peak_dbfs? - self.loudness.mean_dbfs?)
    }
}

/// A whole signal and its sections.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Everything, as one span. This is the number [`super::Meter`] alone would
    /// have produced, and it stays first because it is what a reader reads
    /// first.
    pub whole: Span,
    /// The stretches it is made of, in order.
    ///
    /// **Empty when there is only one**, which is the honest answer rather than
    /// a missing feature: a single row under a one-line summary is the same
    /// sentence twice, and a report that repeats itself trains its reader to
    /// skip both halves.
    pub sections: Vec<Span>,
}

impl Profile {
    /// How loud the whole thing was — the one number a caller that predates
    /// sections still wants.
    pub fn loudness(&self) -> &Loudness {
        &self.whole.loudness
    }

    /// How long the signal runs.
    pub fn seconds(&self) -> f64 {
        self.whole.to_seconds
    }
}

/// Accumulates a [`Profile`] over samples arriving a run at a time.
#[derive(Debug, Clone)]
pub struct Profiler {
    channels: usize,
    rate: u32,
    whole: Section,
    current: Section,
    done: Vec<Span>,
    /// Frames taken so far, which is where in the signal the next run starts.
    fed: u64,
    /// The boundaries still ahead, nearest first. Once these run out the
    /// fixed interval takes over, which is what carries a song's ring-out past
    /// its last pattern.
    cuts: VecDeque<Cut>,
}

impl Profiler {
    /// A profiler that cuts on a fixed interval — for anything whose document
    /// does not say where its sections are.
    pub fn new(channels: usize, rate: u32) -> Self {
        Self::sectioned(channels, rate, Vec::new())
    }

    /// A profiler that cuts where an arrangement says, and on the fixed
    /// interval after the last of them.
    pub fn sectioned(channels: usize, rate: u32, cuts: Vec<Cut>) -> Self {
        let channels = channels.max(1);
        Self {
            channels,
            rate,
            whole: Section::new(channels, rate, None, 0),
            current: Section::new(channels, rate, label_of(cuts.first()), 0),
            done: Vec::new(),
            fed: 0,
            cuts: cuts.into(),
        }
    }

    /// Takes another run of interleaved samples, splitting it wherever a
    /// section boundary falls inside it.
    pub fn feed(&mut self, samples: &[f32]) {
        let mut rest = samples;
        while !rest.is_empty() {
            let room = self.until_boundary();
            if room == 0 {
                self.close();
                continue;
            }
            let take = rest.len().min(room);
            self.whole.feed(&rest[..take]);
            self.current.feed(&rest[..take]);
            self.fed += (take / self.channels) as u64;
            rest = &rest[take..];
        }
    }

    /// What it all came out as.
    pub fn finish(&self) -> Profile {
        let mut sections = self.done.clone();
        if self.fed > self.current.from {
            sections.push(self.current.close(self.fed));
        }
        // One section is the whole file said twice; see [`Profile::sections`].
        if sections.len() < 2 {
            sections.clear();
        }
        Profile {
            whole: self.whole.close(self.fed),
            sections,
        }
    }

    /// How many samples may still be fed before the next boundary, as a count
    /// of interleaved samples rather than frames.
    ///
    /// Saturating throughout, because a boundary is arithmetic on a number out
    /// of a document: a pattern of `1e30` beats is a legal `f32` and would
    /// otherwise overflow this into a panic, which is a poor way for a report
    /// to report anything.
    fn until_boundary(&self) -> usize {
        let boundary = match self.cuts.front() {
            Some(cut) => self.frame_at(cut.end_seconds),
            None => self
                .current
                .from
                .saturating_add(self.frame_at(FALLBACK_SECONDS).max(1)),
        };
        usize::try_from(boundary.saturating_sub(self.fed))
            .unwrap_or(usize::MAX)
            .saturating_mul(self.channels)
    }

    /// Which sample-frame a time in seconds falls on.
    fn frame_at(&self, seconds: f64) -> u64 {
        (seconds * f64::from(self.rate)).round().max(0.0) as u64
    }

    /// Ends the section in progress and starts the next one.
    fn close(&mut self) {
        // A boundary that lands where the last one did — two patterns of zero
        // beats, say — must not produce an empty row, and must still be
        // consumed or the loop that called this would never advance.
        if self.fed > self.current.from {
            self.done.push(self.current.close(self.fed));
        }
        self.cuts.pop_front();
        self.current = Section::new(
            self.channels,
            self.rate,
            label_of(self.cuts.front()),
            self.fed,
        );
    }
}

/// The label a boundary gives the stretch that ends at it.
fn label_of(cut: Option<&Cut>) -> Option<String> {
    cut.map(|cut| cut.label.clone())
}

/// One stretch being measured: the two meters, and where it began.
#[derive(Debug, Clone)]
struct Section {
    label: Option<String>,
    from: u64,
    rate: u32,
    level: Meter,
    bands: BandMeter,
}

impl Section {
    fn new(channels: usize, rate: u32, label: Option<String>, from: u64) -> Self {
        Self {
            label,
            from,
            rate,
            level: Meter::new(channels),
            bands: BandMeter::new(channels, rate),
        }
    }

    fn feed(&mut self, samples: &[f32]) {
        self.level.feed(samples);
        self.bands.feed(samples);
    }

    fn close(&self, until: u64) -> Span {
        Span {
            label: self.label.clone(),
            from_seconds: self.seconds(self.from),
            to_seconds: self.seconds(until),
            loudness: self.level.finish(),
            bands: self.bands.finish(),
            correlation: self.level.correlation(),
        }
    }

    fn seconds(&self, frame: u64) -> f64 {
        if self.rate == 0 {
            return 0.0;
        }
        frame as f64 / f64::from(self.rate)
    }
}
