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
//! These are plain serde data (no audio-device types), so they serialize straight
//! into the agent snapshot and a scenario can assert on them with no sound card.

use serde::{Deserialize, Serialize};

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
/// so a scenario can assert ordering without any wall clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioEvent {
    pub kind: AudioEventKind,
    /// The clip path that played (empty for a `Stop` of a clip-less source).
    pub clip: String,
    /// The source entity id. For `PlayAt` this is the caller-supplied source (or
    /// `0` when none was given) — useful to attribute a one-shot to its emitter.
    pub entity: u32,
    /// World position the sound played at. For an entity source this is its
    /// transform position; for `PlayAt` it is the explicit point. (Stored for #213;
    /// 2D playback ignores it but the agent can still read where it happened.)
    pub position: [f32; 3],
    /// Effective volume the action requested (pre-master).
    pub volume: f32,
    /// The play-mode frame the action happened on.
    pub tick: u64,
}

/// A bounded ring buffer of [`AudioEvent`]s. Old entries fall off past the cap so an
/// hour-long play-test can't grow the log without bound; the agent reads the tail.
#[derive(Clone, Debug, Default)]
pub struct AudioEventLog {
    events: Vec<AudioEvent>,
    cap: usize,
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
        }
    }

    /// Append an event, evicting the oldest if the cap is reached (FIFO).
    pub fn push(&mut self, event: AudioEvent) {
        if self.events.len() >= self.cap {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// Every logged event, oldest first.
    pub fn events(&self) -> &[AudioEvent] {
        &self.events
    }

    /// Drop the whole log (e.g. on entering Play for a clean record).
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Number of logged events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(tick: u64) -> AudioEvent {
        AudioEvent {
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
}
