use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogSettings {
    /// The log's size budget in megabytes; the oldest segment is dropped past it.
    /// `0` writes no log.
    pub budget_mb: u32,
    pub minutes: Option<u32>,
    pub editor: bool,
    pub player: bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogChannels {
    pub frames: bool,
    /// The GPU ms per render-pass kind, on a frame record.
    pub gpu_passes: bool,
    pub systems: bool,
    /// `Debug.Log`, warnings and errors.
    pub console: bool,
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
    /// In the order `docs/api/Debug.md`'s channel table lists them.
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

    pub fn get(&self, name: &str) -> Option<bool> {
        let mut copy = *self;
        copy.slot(name).map(|on| *on)
    }

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
