//! `f64` inside: a piece lasts minutes and a frame 23 µs, more than `f32` holds in one number.

use super::super::tempo::TempoChange;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Map {
    // Never empty, the first starting on beat 0, so every position from 0 on has a segment.
    segments: Vec<Segment>,
    period: f64,
    pass: f64,
    // Each pass replays the map from beat 0, so an accelerando restarts and every pass lasts
    // `pass` seconds.
    passes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Segment {
    from: f64,
    at: f64,
    bpm: f64,
    slope: f64,
}

impl Segment {
    fn seconds(self, beats: f64) -> f64 {
        if self.slope == 0.0 {
            beats * 60.0 / self.bpm
        } else {
            60.0 / self.slope * (self.slope * beats / self.bpm).ln_1p()
        }
    }

    fn beats(self, seconds: f64) -> f64 {
        if self.slope == 0.0 {
            seconds * self.bpm / 60.0
        } else {
            self.bpm / self.slope * (self.slope * seconds / 60.0).exp_m1()
        }
    }
}

impl Map {
    /// `changes` passed `tempo::check`: beats after 0 and ascending, every tempo positive.
    pub(super) fn new(
        bpm: f32,
        changes: &[TempoChange],
        scale: f64,
        period: f32,
        passes: u32,
    ) -> Self {
        let mut segments = Vec::with_capacity(changes.len() + 1);
        let (mut from, mut at, mut tempo) = (0.0, 0.0, f64::from(bpm) * scale);
        for change in changes {
            let beat = f64::from(change.beat);
            let target = f64::from(change.bpm) * scale;
            let slope = if change.ramp {
                (target - tempo) / (beat - from)
            } else {
                0.0
            };
            let segment = Segment {
                from,
                at,
                bpm: tempo,
                slope,
            };
            at += segment.seconds(beat - from);
            segments.push(segment);
            (from, tempo) = (beat, target);
        }
        segments.push(Segment {
            from,
            at,
            bpm: tempo,
            slope: 0.0,
        });
        let mut map = Self {
            segments,
            period: f64::from(period),
            pass: 0.0,
            passes: passes.max(1),
        };
        map.pass = map.seconds_within(map.period);
        map
    }

    pub(super) fn anchored(&self, landings: &[(f64, f64)]) -> Self {
        let mut bounds = vec![(0.0, 0.0)];
        bounds.extend_from_slice(landings);
        let mut segments = Vec::new();
        for (index, &(from, at)) in bounds.iter().enumerate() {
            let (until, scale) = match bounds.get(index + 1) {
                Some(&(beat, seconds)) => (
                    beat,
                    (self.seconds_within(beat) - self.seconds_within(from)) / (seconds - at),
                ),
                None => (f64::INFINITY, 1.0),
            };
            let base = self.seconds_within(from);
            for (next, segment) in self.segments.iter().enumerate() {
                let end = self
                    .segments
                    .get(next + 1)
                    .map_or(f64::INFINITY, |it| it.from);
                if end <= from || segment.from >= until {
                    continue;
                }
                let start = segment.from.max(from);
                segments.push(Segment {
                    from: start,
                    at: at + (self.seconds_within(start) - base) / scale,
                    bpm: (segment.bpm + segment.slope * (start - segment.from)) * scale,
                    slope: segment.slope * scale,
                });
            }
        }
        let mut map = Self {
            segments,
            period: self.period,
            pass: 0.0,
            passes: 1,
        };
        map.pass = map.seconds_within(map.period);
        map
    }

    pub(super) fn opening(&self) -> f64 {
        self.segments[0].bpm
    }

    pub(super) fn slowest(&self) -> f64 {
        let ends = self.segments.iter().enumerate().map(|(index, it)| {
            let end = self
                .segments
                .get(index + 1)
                .map_or(it.from, |next| next.from);
            (it.bpm + it.slope * (end - it.from)).min(it.bpm)
        });
        ends.fold(f64::INFINITY, f64::min)
    }

    pub(super) fn tempo(&self, beats: f64) -> f64 {
        let pass = self.pass_of(beats / self.period);
        let beat = (beats - pass * self.period).max(0.0);
        let segment = self.segment(self.segments.partition_point(|it| it.from <= beat));
        segment.bpm + segment.slope * (beat - segment.from)
    }

    pub(super) fn seconds(&self, beats: f64) -> f64 {
        let pass = self.pass_of(beats / self.period);
        pass * self.pass + self.seconds_within(beats - pass * self.period)
    }

    pub(super) fn beats(&self, seconds: f64) -> f64 {
        let pass = self.pass_of(seconds / self.pass);
        pass * self.period + self.beats_within(seconds - pass * self.pass)
    }

    /// The last pass keeps everything past its end, so a ring-out runs at the final tempo.
    fn pass_of(&self, passes: f64) -> f64 {
        if passes.is_finite() {
            passes.floor().clamp(0.0, f64::from(self.passes - 1))
        } else {
            0.0
        }
    }

    fn seconds_within(&self, beat: f64) -> f64 {
        let beat = beat.max(0.0);
        let segment = self.segment(self.segments.partition_point(|it| it.from <= beat));
        segment.at + segment.seconds(beat - segment.from)
    }

    fn beats_within(&self, seconds: f64) -> f64 {
        let seconds = seconds.max(0.0);
        let segment = self.segment(self.segments.partition_point(|it| it.at <= seconds));
        segment.from + segment.beats(seconds - segment.at)
    }

    fn segment(&self, after: usize) -> Segment {
        self.segments[after.saturating_sub(1)]
    }
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
