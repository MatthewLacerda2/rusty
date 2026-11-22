//! Defects that live between samples: clicks, dropouts and stepped gain, each at a moment.
//! Stepped gain errs toward silence: a sound whose envelope pulses on its own is not judged, so
//! check a fix on a steady sound.

mod clicks;
mod envelope;
mod quadrature;
mod zipper;

use clicks::Clicks;
use envelope::Envelope;

/// Kept per kind, worst first; the rest are only counted.
pub const WORST: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct Defect {
    /// `None` when the defect is in every channel (always, in a mono file); the lowest channel it
    /// is in when that is only some of them.
    pub channel: Option<usize>,
    pub at_seconds: f64,
    pub from_seconds: f64,
    pub to_seconds: f64,
    pub size_db: f64,
    pub period_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tally {
    pub count: usize,
    pub worst: Vec<Defect>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Defects {
    pub clicks: Tally,
    pub dropouts: Tally,
    pub zipper: Tally,
}

impl Defects {
    pub fn is_clean(&self) -> bool {
        self.clicks.count + self.dropouts.count + self.zipper.count == 0
    }
}

#[derive(Debug, Clone)]
pub struct DefectFinder {
    channels: usize,
    rate: u32,
    clicks: Vec<Clicks>,
    envelopes: Vec<Envelope>,
    next: usize,
}

impl DefectFinder {
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

    pub fn feed(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.clicks[self.next].feed(sample);
            self.envelopes[self.next].feed(sample);
            self.next = (self.next + 1) % self.channels;
        }
    }

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

    fn tally(&self, mut found: Vec<Defect>) -> Tally {
        found.sort_by(|a, b| a.from_seconds.total_cmp(&b.from_seconds));
        let mut joined: Vec<(Defect, Vec<usize>)> = Vec::new();
        for one in found {
            let channel = one.channel.unwrap_or(0);
            match joined.last_mut() {
                // Only the same defect in another channel joins: a channel's finder kept its own
                // apart.
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
