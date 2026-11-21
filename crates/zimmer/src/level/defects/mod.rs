//! Defects that live between samples: clicks, dropouts and stepped gain, each
//! at a moment.
//!
//! Level, bands and the per-section profile describe how a sound *sits*; none
//! of them can tell a clean fade from a staircase of gain steps, or find a
//! 10 ms gap inside a one-second section, because both average away inside
//! any window long enough to measure loudness over. These are the defects a
//! listener calls "choppy" or "crackly", and they are found here by shape:
//!
//! - **A click** is one sample-to-sample jump far larger than any other jump
//!   around it ([`clicks`]). *Any other jump*, not the average one: a sawtooth
//!   jumps every period and is not clicking, and noise is nothing but jumps.
//! - **A dropout** is a short run of near-silence inside sound that is going
//!   on either side of it ([`envelope`]). Short is the point: a pause between
//!   sentences is longer than any dropout.
//! - **Stepped gain** ("zipper") is a level that moves in flat steps at a
//!   regular period — a gain set once per game frame and held — instead of
//!   smoothly, reported with the period found ([`zipper`]), from 5 ms to
//!   64 ms and from a quarter of a decibel a step.
//!
//! **Stepped gain errs toward silence.** It is read off the sound's envelope
//! ([`quadrature`]), so a steady tone, a chord or noise shows a staircase
//! clearly, but a sound whose envelope pulses on its own — a voice, a note
//! rich in harmonics — is not judged at all: its pulse and a staircase cannot
//! be told apart there, and a measurement that invented staircases in every
//! narration line would be worse than none. Nor is a step period shorter than
//! about two cycles of the sound, which holds too little of it to have a
//! level. To check a fix for stepped gain, measure a steady sound through it.
//!
//! **A signal, never a gate**, like the rest of `level`: a hard cut in an edit
//! is a click on purpose, and a gated drum is a dropout on purpose. What this
//! changes is that "it sounds choppy" becomes a time, a channel and a number
//! that a fix can be measured against.
//!
//! **Fed a run at a time** and never holding the signal: the click search
//! keeps a window of 50 ms per channel, and the other two keep two energies
//! per half millisecond, a sixth of the samples' own size.

mod clicks;
mod envelope;
mod quadrature;
mod zipper;

use clicks::Clicks;
use envelope::Envelope;

/// How many instances of each kind a report keeps, worst first. The rest are
/// counted: the worst few are what anybody listens to, and a broken capture
/// with ten thousand clicks must not produce a report ten thousand lines long.
pub const WORST: usize = 5;

/// One defect, where it is and how bad.
#[derive(Debug, Clone, PartialEq)]
pub struct Defect {
    /// The channel it is in, from zero; `None` when it is in every channel at
    /// once, which is also how a mono file's defects read. In some channels
    /// of a file of more than two, the lowest of them.
    pub channel: Option<usize>,
    /// The moment to look at, in seconds: the click itself, the start of a
    /// gap, the first step of a staircase.
    pub at_seconds: f64,
    /// Where the stretch it occupies starts, in seconds: a few milliseconds
    /// either side of a click, the gap of a dropout, first step to last of a
    /// staircase.
    pub from_seconds: f64,
    /// Where that stretch ends, in seconds.
    pub to_seconds: f64,
    /// How bad, in decibels: how far a click's jump stands above every other
    /// jump around it, how far a dropout falls under the sound either side, and
    /// the typical size of one step of a staircase.
    pub size_db: f64,
    /// How often a staircase steps, in seconds — `1/60` is a 60 Hz control
    /// rate. `None` for a click and a dropout.
    pub period_seconds: Option<f64>,
}

/// One kind of defect: how many, and the worst of them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tally {
    /// How many were found.
    pub count: usize,
    /// The worst [`WORST`] of them, the largest [`Defect::size_db`] first.
    pub worst: Vec<Defect>,
}

/// Every defect found in a signal, by kind.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Defects {
    /// Sample-to-sample jumps far above every jump around them.
    pub clicks: Tally,
    /// Short runs of near-silence inside continuous sound.
    pub dropouts: Tally,
    /// Runs of a level moving in flat steps at a regular period.
    pub zipper: Tally,
}

impl Defects {
    /// True when nothing of any kind was found.
    pub fn is_clean(&self) -> bool {
        self.clicks.count + self.dropouts.count + self.zipper.count == 0
    }
}

/// Finds [`Defects`] in a signal arriving a run at a time.
#[derive(Debug, Clone)]
pub struct DefectFinder {
    channels: usize,
    rate: u32,
    clicks: Vec<Clicks>,
    envelopes: Vec<Envelope>,
    /// Which channel the next interleaved sample belongs to, so a run may end
    /// part-way through a frame.
    next: usize,
}

impl DefectFinder {
    /// A finder for a signal of `channels` interleaved channels at `rate`
    /// samples a second.
    pub fn new(channels: usize, rate: u32) -> Self {
        let channels = channels.max(1);
        let rate = rate.max(1);
        Self {
            channels,
            rate,
            clicks: (0..channels).map(|_| Clicks::new(rate)).collect(),
            envelopes: (0..channels).map(|_| Envelope::new(rate)).collect(),
            next: 0,
        }
    }

    /// Takes another run of interleaved samples.
    pub fn feed(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.clicks[self.next].feed(sample);
            self.envelopes[self.next].feed(sample);
            self.next = (self.next + 1) % self.channels;
        }
    }

    /// What was found in the signal so far.
    ///
    /// Read rather than consumed, as [`super::Profiler::finish`] is: the last
    /// few milliseconds are judged as if the signal ended here.
    pub fn finish(&self) -> Defects {
        let mut clicks = Vec::new();
        let mut dropouts = Vec::new();
        let mut zipper = Vec::new();
        for (channel, (found, envelope)) in self.clicks.iter().zip(&self.envelopes).enumerate() {
            let seconds = |frame: u64| frame as f64 / f64::from(self.rate);
            clicks.extend(
                found
                    .finish()
                    .into_iter()
                    .map(|one| one.at(channel, seconds)),
            );
            dropouts.extend(
                envelope
                    .dropouts()
                    .into_iter()
                    .map(|one| one.at(channel, seconds)),
            );
            zipper.extend(
                envelope
                    .zipper()
                    .into_iter()
                    .map(|one| one.at(channel, seconds)),
            );
        }
        Defects {
            clicks: self.tally(clicks),
            dropouts: self.tally(dropouts),
            zipper: self.tally(zipper),
        }
    }

    /// Joins the same defect seen in several channels into one, and keeps the
    /// worst. Two in one channel are never joined, however close: that
    /// channel's own finder already decided they are two.
    fn tally(&self, mut found: Vec<Defect>) -> Tally {
        found.sort_by(|a, b| a.from_seconds.total_cmp(&b.from_seconds));
        let mut joined: Vec<(Defect, Vec<usize>)> = Vec::new();
        for one in found {
            let channel = one.channel.unwrap_or(0);
            match joined.last_mut() {
                // Only the same defect seen in another channel is joined: a
                // channel's own finder has already kept its defects apart.
                Some((last, channels))
                    if one.from_seconds < last.to_seconds && !channels.contains(&channel) =>
                {
                    last.to_seconds = last.to_seconds.max(one.to_seconds);
                    if one.size_db > last.size_db {
                        last.at_seconds = one.at_seconds;
                        last.size_db = one.size_db;
                        last.period_seconds = one.period_seconds;
                    }
                    if !channels.contains(&channel) {
                        channels.push(channel);
                    }
                }
                _ => joined.push((one, vec![channel])),
            }
        }
        let count = joined.len();
        let mut worst: Vec<Defect> = joined
            .into_iter()
            .map(|(mut one, channels)| {
                one.channel = (channels.len() < self.channels)
                    .then(|| channels.into_iter().min().unwrap_or(0));
                one
            })
            .collect();
        worst.sort_by(|a, b| b.size_db.total_cmp(&a.size_db));
        worst.truncate(WORST);
        Tally { count, worst }
    }
}

/// A defect found in one channel, in frames, before it is placed in time.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Found {
    at: u64,
    from: u64,
    to: u64,
    size_db: f64,
    period: Option<f64>,
}

impl Found {
    fn at(self, channel: usize, seconds: impl Fn(u64) -> f64) -> Defect {
        Defect {
            channel: Some(channel),
            at_seconds: seconds(self.at),
            from_seconds: seconds(self.from),
            to_seconds: seconds(self.to),
            size_db: self.size_db,
            period_seconds: self.period.map(|frames| frames * seconds(1)),
        }
    }
}
