use serde::{Deserialize, Serialize};

use super::AudioMaestro;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialResult {
    pub gain: f32,
    pub pan: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub entity: u32,
    pub clip: String,
    pub volume: f32,
    pub looping: bool,
    pub is_time_scaled: bool,
    pub playing: bool,
    pub spatial: SpatialResult,
    pub occlusion: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioEventKind {
    Play,
    Stop,
    PlayAt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioEvent {
    /// The log's sequence number: from 1, one per logged event, never reused while
    /// the process lives (a clear on Play keeps counting). Set by
    /// [`AudioEventLog::push`], which overwrites whatever the caller passed.
    pub seq: u64,
    pub kind: AudioEventKind,
    pub clip: String,
    pub entity: u32,
    #[serde(with = "xyz")]
    pub position: [f32; 3],
    pub volume: f32,
    pub tick: u64,
}

#[derive(Clone, Debug)]
pub struct AudioEventLog {
    events: Vec<AudioEvent>,
    cap: usize,
    next_seq: u64,
}

impl Default for AudioEventLog {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_CAP)
    }
}

pub const DEFAULT_EVENT_CAP: usize = 1024;

impl AudioEventLog {
    pub fn new(cap: usize) -> Self {
        Self {
            events: Vec::new(),
            cap: cap.max(1),
            next_seq: 1,
        }
    }

    pub fn push(&mut self, mut event: AudioEvent) {
        if self.events.len() >= self.cap {
            self.events.remove(0);
        }
        event.seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(event);
    }

    pub fn events(&self) -> &[AudioEvent] {
        &self.events
    }

    pub fn after(&self, after_seq: u64) -> &[AudioEvent] {
        let from = self.events.partition_point(|e| e.seq <= after_seq);
        &self.events[from..]
    }

    pub fn oldest_seq(&self) -> u64 {
        self.events.first().map_or(self.next_seq, |e| e.seq)
    }

    /// Drop the whole log (e.g. on entering Play for a clean record). `seq` keeps
    /// counting, so a reader's cursor never matches an event of the new run twice.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl AudioMaestro {
    pub fn events(&self) -> &[AudioEvent] {
        self.log.events()
    }

    pub fn events_after(&self, after_seq: u64) -> (&[AudioEvent], u64) {
        (self.log.after(after_seq), self.log.oldest_seq())
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}

mod xyz {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Xyz {
        x: f32,
        y: f32,
        z: f32,
    }

    pub fn serialize<S: Serializer>(p: &[f32; 3], s: S) -> Result<S::Ok, S::Error> {
        let [x, y, z] = *p;
        Xyz { x, y, z }.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 3], D::Error> {
        let Xyz { x, y, z } = Xyz::deserialize(d)?;
        Ok([x, y, z])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(tick: u64) -> AudioEvent {
        AudioEvent {
            seq: 0,
            kind: AudioEventKind::Play,
            clip: "a.ogg".to_string(),
            entity: 1,
            position: [0.0, 0.0, 0.0],
            volume: 1.0,
            tick,
        }
    }

    #[test]
    fn log_evicts_oldest_past_cap() {
        let mut log = AudioEventLog::new(2);
        log.push(ev(1));
        log.push(ev(2));
        log.push(ev(3));
        assert_eq!(log.len(), 2);
        assert_eq!(log.events()[0].tick, 2);
        assert_eq!(log.events()[1].tick, 3);
    }

    #[test]
    fn zero_cap_is_clamped_to_one() {
        let mut log = AudioEventLog::new(0);
        log.push(ev(1));
        log.push(ev(2));
        assert_eq!(log.len(), 1);
        assert_eq!(log.events()[0].tick, 2);
    }

    #[test]
    fn clear_empties_the_log() {
        let mut log = AudioEventLog::new(8);
        log.push(ev(1));
        assert!(!log.is_empty());
        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn seq_counts_from_one_and_survives_a_clear() {
        let mut log = AudioEventLog::new(8);
        log.push(ev(1));
        log.push(ev(1));
        assert_eq!(log.after(0).len(), 2);
        assert_eq!(log.after(1)[0].seq, 2);
        log.clear();
        assert_eq!(log.oldest_seq(), 3, "an empty log names the next seq");
        log.push(ev(0));
        assert_eq!(log.after(2)[0].seq, 3, "a new run keeps counting");
    }

    #[test]
    fn oldest_seq_moves_past_what_rolled_off() {
        let mut log = AudioEventLog::new(2);
        (0..5).for_each(|t| log.push(ev(t)));
        assert_eq!(log.oldest_seq(), 4);
        assert_eq!(log.after(0).len(), 2, "only what is held comes back");
    }
}
