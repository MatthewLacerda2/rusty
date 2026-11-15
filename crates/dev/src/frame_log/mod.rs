//! crates/dev/src/frame_log/ — the rolling frame and console log on disk (#2206).
//!
//! Unity keeps `Editor.log` on disk and its Profiler a ring of recent frames; this is
//! both for an agent that reads files: while the window runs (the editor, on by
//! default; a `player` dev build, off by default), one JSON line per drawn frame and
//! per console line, audio event and finished load goes under the project's
//! `saved/logs/`, so "a minute ago, by the bar" can be found after the fact with
//! nothing armed beforehand. `Debug.FrameLog{ … }` reads a slice back ([`read()`]).
//!
//! * **Channels** (`LogChannels`, kept in `project.rusty`): `frames` (one record a
//!   drawn frame), `gpu_passes` and `systems` (fields on it), `console`, `loads`,
//!   `audio` (records of their own). Read each frame, so a switch takes effect at once.
//! * **Bounded**: numbered segments `frames-<n>.jsonl`, a tenth of the budget each;
//!   past the budget (or, when set, the period) the oldest segment is deleted
//!   ([`writer`]). The budget is read when the log starts.
//! * **Never stalls a frame**: the frame hands its lines to a writer thread through
//!   a bounded channel; when the writer is behind, the frame's lines are dropped and
//!   counted, and the next write says how many (`{"ch":"dropped",…}`).
//! * **A cut is survivable**: a segment is synced before the next opens, so after a
//!   power cut every earlier segment reads whole; the reader skips a cut line.
//! * **Display-side**: wall-clock time is written here, in the dev layer, and nothing
//!   the sim reads changes.
//!
//! One record (a frame, wrapped here):
//!
//! ```json
//! {"ch":"frames","t":1763190000123,"tick":812,"play":true,"scene":"assets/scenes/a.scene",
//!  "cam":[1.0,1.7,-4.2],"fwd":[0.0,0.0,1.0],"ms":16.7,"fps":60.0,"ticks":1,"sim_ms":2.1,
//!  "cpu_ms":3.0,"gpu_ms":5.2,"draws":412,"tris":180344,"visible":300,"culled":120,
//!  "gpu_passes":{"forward":2.1,"shadow":1.4},"systems":{"step_physics":0.9}}
//! ```
//!
//! stored-format: `saved/logs/frames-<n>.jsonl`, the rolling frame log

use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_app::GameWorld;
use rusty_core::settings::logs::LogSettings;

use crate::stats::WindowFrame;

pub mod read;
mod record;
pub mod writer;

pub use read::{read, LogQuery, LogRead};

/// The project folder the log is written to (inside the git-ignored `saved/`).
pub const LOGS_DIR: &str = "saved/logs";

/// A system is written on a frame record when its CPU time over the frame's ticks
/// reaches this many ms: the ones worth a look, not the dozen that cost nothing.
pub const SYSTEM_MS: f64 = 0.25;

/// Which host writes: each has its own switch in the project's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogHost {
    Editor,
    Player,
}

/// A running log: the writer thread, and the cursors that keep each console line
/// and audio event to one record.
pub struct FrameLog {
    writer: writer::Writer,
    console_mark: Option<u64>,
    audio_seq: u64,
}

impl FrameLog {
    /// Start the project's log in [`LOGS_DIR`] when `settings` say `host` writes
    /// one; `None` (and a warning on failure) otherwise.
    pub fn for_host(host: LogHost, settings: LogSettings) -> Option<Self> {
        let on = match host {
            LogHost::Editor => settings.editor,
            LogHost::Player => settings.player,
        };
        on.then(|| Self::start(Path::new(LOGS_DIR), settings))
            .flatten()
    }

    /// Start a log in `dir` under `settings`' budget and period; `None` when the
    /// budget is 0 or the folder cannot be made.
    pub fn start(dir: &Path, settings: LogSettings) -> Option<Self> {
        if settings.budget_mb == 0 {
            return None;
        }
        let budget = u64::from(settings.budget_mb) * 1024 * 1024;
        let period = settings.minutes.map(|m| u64::from(m) * 60_000);
        match writer::Writer::start(dir.to_path_buf(), budget, period) {
            Ok(writer) => Some(Self {
                writer,
                console_mark: None,
                audio_seq: 0,
            }),
            Err(err) => {
                log::warn!("[FrameLog] cannot start in {}: {err}", dir.display());
                None
            }
        }
    }

    /// The folder this log writes.
    pub fn dir(&self) -> &PathBuf {
        self.writer.dir()
    }

    /// Records the writer dropped so far because it was behind.
    pub fn dropped(&self) -> u64 {
        self.writer.dropped()
    }

    /// Log one drawn frame of `game`: its record (when `frames` is on), and the
    /// console lines, audio events and loads since the last one, per the
    /// project's channels as they stand now. Call it after the frame's stats were
    /// recorded, so the newest frame of the stats is this one.
    pub fn frame(&mut self, game: &GameWorld, frame: &WindowFrame) {
        let channels = game.resources.application.borrow().log_settings().channels;
        let place = OnceCell::new();
        let place = || place.get_or_init(|| record::Place::of(game, now_ms()));
        let mut lines = Vec::new();
        if channels.frames || channels.loads {
            let last = game.script_manager().stats_cell().borrow().last_frame();
            if channels.frames {
                lines.push(record::frame(place(), frame, &last, channels));
            }
            if channels.loads {
                lines.extend(record::load(place(), &last));
            }
        }
        let console = game.console().borrow();
        let mark = self.console_mark.replace(console.logged());
        if let (true, Some(mark)) = (channels.console, mark) {
            lines.extend(
                console
                    .since(mark)
                    .map(|(msg, lv)| record::console(place(), msg, lv)),
            );
        }
        let audio = game.resources.audio.borrow();
        let (events, _) = audio.events_after(self.audio_seq);
        if let Some(newest) = events.last() {
            self.audio_seq = newest.seq;
            if channels.audio {
                lines.extend(events.iter().filter_map(|e| record::audio(place(), e)));
            }
        }
        self.writer.send(lines);
    }
}

/// Milliseconds since the Unix epoch, the wall clock a record carries.
pub fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "tests.rs"]
mod frame_log_tests;
