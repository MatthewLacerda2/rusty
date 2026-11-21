//! Where a beat falls in time: the one conversion from a song's beats to seconds of the piece it
//! renders to.

mod map;

use super::anchor::Landing;
use super::Song;
use crate::core::RATE;
use map::Map;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Clock {
    bpm: f32,
    beat: f32,
    per_frame: f32,
    map: Option<Map>,
}

impl Clock {
    pub(crate) fn at(bpm: f32) -> Self {
        Self {
            bpm,
            beat: 60.0 / bpm,
            per_frame: bpm / (60.0 * RATE),
            map: None,
        }
    }

    pub(crate) fn written(song: &Song, passes: u32) -> Self {
        if song.tempo.is_empty() {
            Self::at(song.bpm)
        } else {
            Self::mapped(song, passes, 1.0)
        }
    }

    pub(crate) fn stretched(song: &Song, passes: u32, seconds: f32) -> Self {
        let beats = song.arrangement_beats();
        if song.tempo.is_empty() {
            return Self::at(beats * passes as f32 * 60.0 / seconds);
        }
        let once = f64::from(Self::written(song, 1).seconds(beats));
        Self::mapped(song, passes, once * f64::from(passes) / f64::from(seconds))
    }

    pub(crate) fn anchored(song: &Song, landings: &[Landing]) -> Self {
        let written = Map::new(song.bpm, &song.tempo, 1.0, song.arrangement_beats(), 1);
        let points: Vec<(f64, f64)> = landings.iter().map(|it| (it.beat, it.seconds)).collect();
        let map = written.anchored(&points);
        Self::at(map.opening() as f32).with(map)
    }

    fn with(self, map: Map) -> Self {
        Self {
            map: Some(map),
            ..self
        }
    }

    fn mapped(song: &Song, passes: u32, scale: f64) -> Self {
        let bpm = (f64::from(song.bpm) * scale) as f32;
        Self {
            map: Some(Map::new(
                song.bpm,
                &song.tempo,
                scale,
                song.arrangement_beats(),
                passes,
            )),
            ..Self::at(bpm)
        }
    }

    pub(crate) fn bpm(&self) -> f32 {
        self.bpm
    }

    pub(crate) fn tempo(&self, beats: f32) -> f32 {
        match &self.map {
            None => self.bpm,
            Some(map) => map.tempo(f64::from(beats)) as f32,
        }
    }

    pub(crate) fn seconds(&self, beats: f32) -> f32 {
        match &self.map {
            None => beats * self.beat,
            Some(map) => map.seconds(f64::from(beats)) as f32,
        }
    }

    pub(crate) fn span(&self, from: f32, beats: f32) -> f32 {
        match &self.map {
            None => beats * self.beat,
            Some(map) => {
                let from = f64::from(from);
                (map.seconds(from + f64::from(beats)) - map.seconds(from)) as f32
            }
        }
    }

    pub(crate) fn beat_at(&self, frame: usize) -> f32 {
        match &self.map {
            None => frame as f32 * self.per_frame,
            Some(map) => map.beats(frame as f64 / f64::from(RATE)) as f32,
        }
    }
}

impl crate::core::lfo::Grid for Clock {
    fn beat_at(&self, frame: usize) -> f32 {
        Clock::beat_at(self, frame)
    }
}

impl crate::fx::Tempo for Clock {
    fn beat_seconds(&self, frame: usize) -> f32 {
        match &self.map {
            None => self.beat,
            Some(map) => (60.0 / map.tempo(map.beats(frame as f64 / f64::from(RATE)))) as f32,
        }
    }

    fn longest_beat(&self) -> f32 {
        match &self.map {
            None => self.beat,
            Some(map) => (60.0 / map.slowest()) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beat_at_120_bpm_is_half_a_second() {
        let clock = Clock::at(120.0);
        assert_eq!(clock.seconds(1.0), 0.5);
        assert_eq!(clock.seconds(8.0), 4.0);
        assert_eq!(clock.span(3.0, 2.0), 1.0);
        assert_eq!(clock.beat_at(RATE as usize), 2.0);
    }

    #[test]
    fn a_beat_lasts_what_the_tempo_says_where_it_is_played() {
        use crate::fx::Tempo;
        let steady = Clock::at(120.0);
        assert_eq!(steady.beat_seconds(123_456), 0.5);
        assert_eq!(steady.longest_beat(), 0.5);
        let song = crate::Song::from_json(
            r#"{"bpm": 120, "tempo": [{"beat": 4, "bpm": 60}], "tracks": [],
                "patterns": {"a": {"beats": 8, "notes": []}}, "arrangement": ["a"]}"#,
        )
        .unwrap();
        let slowing = Clock::written(&song, 1);
        assert_eq!(slowing.beat_seconds(RATE as usize), 0.5, "beat 2, at 120");
        assert_eq!(
            slowing.beat_seconds(3 * RATE as usize),
            1.0,
            "beat 5, at 60"
        );
        assert_eq!(slowing.longest_beat(), 1.0);
    }
}
