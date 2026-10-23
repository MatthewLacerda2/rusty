//! src/audio/introspection.rs — agent-facing audio introspection (#212).
//!
//! A headless play-test must be able to ask "what played, where, and by whom" — so
//! the `AudioMaestro` keeps two structured records the agent can read back:
//!
//!   * a **roster** of live per-entity `AudioSource` voices (who is playing what,
//!     at what volume, looping or not, time-scaled or wall-clock), and
//!   * an **event log** of every play action, including fire-and-forget `PlayAt`
//!     one-shots — which leave no persistent component, so the log is the *only*
//!     record they ever played.
//!
//! These are plain serde data (no audio-device types), so a test can assert on them
//! with no sound card (`AudioMaestro::events`). The dev-only Lua reads
//! `Debug.AudioEvents([after_seq])` and `Debug.AudioVoices()` hand both to an agent
//! (`dev::lua_surface::debug::audio`, #1247): `Debug.Snapshot` shows each entity's
//! `AudioSource` component, which a fire-and-forget one-shot never has.

use serde::{Deserialize, Serialize};

use super::AudioMaestro;

/// The resolved spatial state of a source against the listener (#213): the per-source
/// **gain** (pre-master, after distance rolloff + `spatial_blend`) and **pan** in
/// `[-1, 1]` (`-1` = full left, `0` = centre, `+1` = full right). Pure data, computed
/// by [`crate::audio::spatial::resolve`] — no audio device involved, so a headless
/// play-test can assert the agent hears a left-placed source quieter and panned left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialResult {
    /// Pre-master linear gain after distance rolloff and the 2D↔3D blend.
    pub gain: f32,
    /// Stereo pan in `[-1, 1]` (left → right), already scaled by `spatial_blend`.
    pub pan: f32,
}

/// One live voice owned by an entity's `AudioSource`, as the agent sees it. A
/// flattened, device-free view rebuilt each query from the live scene + the
/// maestro's playing set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceInfo {
    /// The owning entity's stable id.
    pub entity: u32,
    /// The clip path this source references.
    pub clip: String,
    /// Effective per-source volume (its own `volume`, pre-master).
    pub volume: f32,
    /// Whether the voice loops.
    pub looping: bool,
    /// Whether the voice respects `Time.timeScale` (vs. wall-clock music/UI).
    pub is_time_scaled: bool,
    /// Whether the maestro currently has this source playing.
    pub playing: bool,
    /// The resolved spatial state — `(gain, pan)` against the listener (#213); the
    /// gain already carries the occlusion muffle.
    pub spatial: SpatialResult,
    /// The smoothed occlusion factor in `[0, 1]` (#467): how much of the source
    /// geometry hides from the listener; `0` when it is not playing.
    pub occlusion: f32,
}

/// What kind of play action an [`AudioEvent`] records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioEventKind {
    /// An entity `AudioSource` started (`Audio.Play` or `play_on_start`).
    Play,
    /// An entity `AudioSource` was explicitly stopped (`Audio.Stop`).
    Stop,
    /// A fire-and-forget one-shot at a world position (`Audio.PlayAt`). Leaves no
    /// persistent component, so this log entry is its only trace.
    PlayAt,
}

/// One logged audio action — the durable "what happened" record, especially for
/// one-shots. `tick` is the play-mode frame the action occurred on (deterministic),
/// so a scenario can assert ordering without any wall clock; `seq` orders the
/// actions *within* a frame and is the reader's cursor (#1247).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioEvent {
    /// The log's sequence number: from 1, one per logged event, never reused while
    /// the process lives (a clear on Play keeps counting). Set by
    /// [`AudioEventLog::push`], which overwrites whatever the caller passed.
    pub seq: u64,
    pub kind: AudioEventKind,
    /// The clip path that played; always `""` for a `Stop`.
    pub clip: String,
    /// The source entity id. For `PlayAt` this is the caller-supplied source (or
    /// `0` when none was given) — useful to attribute a one-shot to its emitter.
    pub entity: u32,
    /// World position the sound played at. For an entity source this is its
    /// transform position; for `PlayAt` it is the explicit point. (Stored for #213;
    /// 2D playback ignores it but the agent can still read where it happened.)
    /// Serializes as a keyed `{ x, y, z }`, the shape the rest of the Lua surface
    /// hands back for a point.
    #[serde(with = "xyz")]
    pub position: [f32; 3],
    /// Effective volume the action requested (pre-master).
    pub volume: f32,
    /// The play-mode frame the action happened on.
    pub tick: u64,
}

/// A bounded ring buffer of [`AudioEvent`]s. Old entries fall off past the cap so an
/// hour-long play-test can't grow the log without bound; the agent reads the tail.
#[derive(Clone, Debug)]
pub struct AudioEventLog {
    events: Vec<AudioEvent>,
    cap: usize,
    /// The `seq` the next pushed event takes.
    next_seq: u64,
}

impl Default for AudioEventLog {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_CAP)
    }
}

/// Default ring capacity — generous enough that a typical scenario reads the whole
/// log, bounded enough that an unattended overnight run can't leak memory.
pub const DEFAULT_EVENT_CAP: usize = 1024;

impl AudioEventLog {
    /// A log holding up to `cap` most-recent events (`cap == 0` is treated as 1).
    pub fn new(cap: usize) -> Self {
        Self {
            events: Vec::new(),
            cap: cap.max(1),
            next_seq: 1,
        }
    }

    /// Append an event under the next `seq`, evicting the oldest if the cap is
    /// reached (FIFO).
    pub fn push(&mut self, mut event: AudioEvent) {
        if self.events.len() >= self.cap {
            self.events.remove(0);
        }
        event.seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(event);
    }

    /// Every logged event, oldest first. Test seam for `tests/audio_api/oneshot.rs`.
    pub fn events(&self) -> &[AudioEvent] {
        &self.events
    }

    /// The held events whose `seq` is greater than `after_seq`, oldest first.
    pub fn after(&self, after_seq: u64) -> &[AudioEvent] {
        let from = self.events.partition_point(|e| e.seq <= after_seq);
        &self.events[from..]
    }

    /// The oldest `seq` still held, or the next one to be assigned when the log is
    /// empty. A reader whose cursor is below `oldest_seq() - 1` missed the events in
    /// between: they rolled off the ring or were cleared on Play.
    pub fn oldest_seq(&self) -> u64 {
        self.events.first().map_or(self.next_seq, |e| e.seq)
    }

    /// Drop the whole log (e.g. on entering Play for a clean record). `seq` keeps
    /// counting, so a reader's cursor never matches an event of the new run twice.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Number of logged events.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the log is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// The maestro's reads of its event log.
impl AudioMaestro {
    /// The agent-facing event log (play / stop / one-shot), oldest first.
    /// Test seam for `tests/audio_api/parented.rs`.
    pub fn events(&self) -> &[AudioEvent] {
        self.log.events()
    }

    /// The held events logged after `after_seq`, and the oldest `seq` still held
    /// (see [`AudioEventLog::after`] and [`AudioEventLog::oldest_seq`]).
    pub fn events_after(&self, after_seq: u64) -> (&[AudioEvent], u64) {
        (self.log.after(after_seq), self.log.oldest_seq())
    }

    /// Clear the event log (e.g. on entering Play for a clean record).
    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}

/// `[x, y, z]` as a keyed `{ x, y, z }` map.
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
