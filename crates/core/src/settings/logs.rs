//! crates/core/src/settings/logs.rs — `LogSettings`: the project's rolling frame log (#2206).
//!
//! A dev build can keep a rolling log of how each drawn frame ran, and what the
//! console said beside it, under `saved/logs/` (`dev::frame_log`), so an agent can
//! look back at "a minute ago" with nothing armed beforehand. The project's
//! `project.rusty` says, under `"logs"`, how much it keeps, which hosts write it and
//! which named channels record. A channel that is off buys the others more history:
//! the budget is shared.

use serde::{Deserialize, Serialize};

/// The project's frame-log settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogSettings {
    /// The log's size budget in megabytes; the oldest segment is dropped past it.
    /// `0` writes no log.
    pub budget_mb: u32,
    /// A second, optional bound: a segment whose newest record is older than this
    /// many minutes is dropped. `None` (or `0`) bounds the log by size alone.
    pub minutes: Option<u32>,
    /// Whether the editor writes it (on by default: a look-back needs no arming).
    pub editor: bool,
    /// Whether a `player` dev build writes it (off by default).
    pub player: bool,
    /// Which channels record.
    pub channels: LogChannels,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            budget_mb: 50,
            minutes: None,
            editor: true,
            player: false,
            channels: LogChannels::default(),
        }
    }
}

impl LogSettings {
    /// The period in ms, `None` when the log is bounded by size alone: `minutes`
    /// unset or `0` (a period of nothing would delete every closed segment).
    pub fn period_ms(&self) -> Option<u64> {
        self.minutes
            .filter(|m| *m > 0)
            .map(|m| u64::from(m) * 60_000)
    }
}

/// The named channels of the frame log, each on or off. `frames` is one record per
/// drawn frame; `gpu_passes` and `systems` ride on it; `console`, `loads` and
/// `audio` are records of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogChannels {
    /// One record per drawn frame: its place, its timings and render counters.
    pub frames: bool,
    /// The GPU ms per render-pass kind, on a frame record.
    pub gpu_passes: bool,
    /// The systems over the log's threshold this frame, on a frame record.
    pub systems: bool,
    /// `Debug.Log`, warnings and errors.
    pub console: bool,
    /// A finished scene load's steps and their times.
    pub loads: bool,
    /// What `Debug.AudioEvents` reads: every play action.
    pub audio: bool,
}

impl Default for LogChannels {
    fn default() -> Self {
        Self {
            frames: true,
            gpu_passes: true,
            systems: true,
            console: true,
            loads: true,
            audio: true,
        }
    }
}

impl LogChannels {
    /// Every channel's name, in the order the docs list them.
    pub const NAMES: [&'static str; 6] = [
        "frames",
        "gpu_passes",
        "systems",
        "console",
        "loads",
        "audio",
    ];

    fn slot(&mut self, name: &str) -> Option<&mut bool> {
        Some(match name {
            "frames" => &mut self.frames,
            "gpu_passes" => &mut self.gpu_passes,
            "systems" => &mut self.systems,
            "console" => &mut self.console,
            "loads" => &mut self.loads,
            "audio" => &mut self.audio,
            _ => return None,
        })
    }

    /// Whether channel `name` records; `None` for a name that is no channel.
    pub fn get(&self, name: &str) -> Option<bool> {
        let mut copy = *self;
        copy.slot(name).map(|on| *on)
    }

    /// Switch channel `name`; an error naming the channels for one that is none.
    pub fn set(&mut self, name: &str, on: bool) -> Result<(), String> {
        match self.slot(name) {
            Some(slot) => {
                *slot = on;
                Ok(())
            }
            None => Err(format!(
                "'{name}' is no log channel (one of {})",
                Self::NAMES.join(", ")
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_is_a_channel_and_others_are_refused() {
        let mut channels = LogChannels::default();
        for name in LogChannels::NAMES {
            assert_eq!(channels.get(name), Some(true), "{name}");
            channels.set(name, false).unwrap();
            assert_eq!(channels.get(name), Some(false), "{name}");
        }
        assert_eq!(channels.get("audi"), None);
        let err = channels.set("audi", true).unwrap_err();
        assert!(err.contains("audi") && err.contains("gpu_passes"), "{err}");
    }

    #[test]
    fn a_partial_block_keeps_the_defaults() {
        let read: LogSettings =
            serde_json::from_str(r#"{"budget_mb": 5, "channels": {"audio": false}}"#).unwrap();
        assert_eq!(read.budget_mb, 5);
        assert!(read.editor && !read.player && read.minutes.is_none());
        assert!(!read.channels.audio && read.channels.frames);
    }

    #[test]
    fn zero_minutes_is_no_period() {
        let mut settings = LogSettings::default();
        assert_eq!(settings.period_ms(), None);
        settings.minutes = Some(0);
        assert_eq!(settings.period_ms(), None, "0 deletes nothing by age");
        settings.minutes = Some(2);
        assert_eq!(settings.period_ms(), Some(120_000));
    }
}
