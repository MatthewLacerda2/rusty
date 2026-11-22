//! The same statistics again, a section at a time.

use std::collections::VecDeque;

use super::bands::{BandMeter, Bands};
use super::meter::{Loudness, Meter};

/// About a phrase: long enough to skip a transient, short enough to split a section that
/// changes midway.
pub(crate) const FALLBACK_SECONDS: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    pub label: String,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub label: Option<String>,
    pub from_seconds: f64,
    pub to_seconds: f64,
    pub loudness: Loudness,
    pub bands: Option<Bands>,
    pub correlation: Option<f64>,
}

impl Span {
    pub fn seconds(&self) -> f64 {
        (self.to_seconds - self.from_seconds).max(0.0)
    }

    pub fn crest_db(&self) -> Option<f64> {
        Some(self.loudness.peak_dbfs? - self.loudness.mean_dbfs?)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub whole: Span,
    pub sections: Vec<Span>,
}

impl Profile {
    pub fn loudness(&self) -> &Loudness {
        &self.whole.loudness
    }

    pub fn seconds(&self) -> f64 {
        self.whole.to_seconds
    }
}

#[derive(Debug, Clone)]
pub struct Profiler {
    channels: usize,
    rate: u32,
    whole: Section,
    current: Section,
    done: Vec<Span>,
    fed: u64,
    /// Once these run out the fallback grid takes over, which carries a song's ring-out past its
    /// last pattern.
    cuts: VecDeque<Cut>,
}

impl Profiler {
    pub fn new(channels: usize, rate: u32) -> Self {
        Self::sectioned(channels, rate, Vec::new())
    }

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

    pub fn finish(&self) -> Profile {
        let mut sections = self.done.clone();
        if self.fed > self.current.from {
            sections.push(self.current.close(self.fed));
        }
        if sections.len() < 2 {
            sections.clear();
        }
        Profile {
            whole: self.whole.close(self.fed),
            sections,
        }
    }

    /// Saturating: a boundary comes from a document, and a 1e30-beat pattern must not overflow
    /// into a panic.
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

    fn frame_at(&self, seconds: f64) -> u64 {
        (seconds * f64::from(self.rate)).round().max(0.0) as u64
    }

    fn close(&mut self) {
        // A boundary at the last one's frame adds no empty row, but is still consumed, or `feed`
        // never advances.
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

fn label_of(cut: Option<&Cut>) -> Option<String> {
    cut.map(|cut| cut.label.clone())
}

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
