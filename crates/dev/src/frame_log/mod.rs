//! crates/dev/src/frame_log/ — the rolling frame and console log on disk (#2206).
//!
//! Unity keeps `Editor.log` on disk and its Profiler a ring of recent frames; this is
//! both for an agent that reads files: while the window runs (the editor, on by
//! default; a `player` dev build, off by default), one JSON line per frame and
//! per console line, audio event and finished load goes under the project's
//! `saved/logs/`, so "a minute ago, by the bar" can be found after the fact with
//! nothing armed beforehand. `Debug.FrameLog{ … }` reads a slice back ([`read()`]).
//!
//! * **Channels** (`LogChannels`, kept in `project.rusty`): `frames` (one record a
//!   drawn frame), `gpu_passes` and `systems` (fields on it), `console`, `loads`,
//!   `audio` (records of their own). Read from the `Application` each frame, so a
//!   switch through `Debug.SetLogChannels` or the Build Settings checkbox takes
//!   effect the next frame; a hand edit of `project.rusty` takes effect at the
//!   next reread of the file, as every project setting does.
//! * **Bounded**: numbered segments `frames-<n>.jsonl`, a tenth of the budget each;
//!   past the budget (or, when set, the period) the oldest segment is deleted
//!   ([`writer`]). The budget is read when the log starts.
//! * **One writer per folder**: a second process on the project finds the
//!   folder's lock held, warns once and logs nothing ([`writer`]).
//! * **Never stalls a frame**: the frame hands its lines to a writer thread through
//!   a bounded channel; when the writer is behind, the frame's lines are dropped and
//!   counted, and the next write says how many (`{"ch":"dropped",…}`), console lines
//!   lost past a frame's cap (`FILE_FEED_CAP`) included.
//! * **Every window frame**, drawn or not: a window that cannot show a frame (a
//!   locked screen) still runs the sim, and its frames and console are logged.
//! * **A cut is survivable**: a segment is synced before the next opens, so after a
//!   power cut every earlier segment reads whole; the reader skips a cut line.
//! * **Display-side**: wall-clock time is written here, in the dev layer, and nothing
//!   the sim reads changes.
//!
//! One record (a frame, wrapped here). Every line opens with `ch`, `t` and `tick`,
//! and a frame's with `frame`, `ms` and `cpu_ms` (when it drew), so a read filters
//! on them without parsing the line (`head`). `frame` is the renderer's drawn-frame
//! count. A line is whole on disk: `ms` is the frame's own wall time, from its
//! start to the next one's, and `gpu_ms` and `gpu_passes` its own GPU times, the
//! writer holding the frame until they are in (`held`); what never arrived is
//! named in `unresolved`:
//!
//! ```json
//! {"ch":"frames","t":1763190000123,"tick":812,"frame":9031,"ms":16.7,
//!  "cpu_ms":3.0,"cam":[1.0,1.7,-4.2],"culled":120,"draws":412,"fps":60.0,
//!  "fwd":[0.0,0.0,1.0],"gpu_ms":5.2,"gpu_passes":{"forward":2.1,"shadows":1.4},
//!  "play":true,"scene":"assets/scenes/a.scene","sim_ms":2.1,
//!  "systems":{"step_physics":0.9},"ticks":1,"tris":180344,"visible":300}
//! ```
//!
//! stored-format: `saved/logs/frames-<n>.jsonl`, the rolling frame log

use std::cell::OnceCell;
use std::path::Path;
use std::sync::PoisonError;
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_app::GameWorld;
use rusty_core::settings::logs::LogSettings;
use rusty_script::scripting::FileLine;

use crate::stats::WindowFrame;

mod head;
mod held;
pub mod read;
mod record;
mod segments;
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

/// A running log: the writer thread, the frames held until whole (`held`), and
/// the cursor that keeps each audio event to one record.
pub struct FrameLog {
    writer: writer::Writer,
    held: held::Shared,
    /// Whether the console feeds this log yet: until it does, the lines it logged
    /// before the first frame (a boot scene's script error) come from its buffer.
    fed: bool,
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
        let period = settings.period_ms();
        match writer::Writer::start(dir, budget, period) {
            Ok(writer) => Some(Self {
                writer,
                held: held::register(dir),
                fed: false,
                audio_seq: 0,
            }),
            Err(err) => {
                log::warn!("[FrameLog] cannot start in {}: {err}", dir.display());
                None
            }
        }
    }

    /// Log one window frame of `game`, drawn or not: the last frame's wall time
    /// (this one's `last_wall_ms`) and any GPU times resolved go onto the frames they
    /// belong to; then this frame's record (when `frames` is on) and the console
    /// lines, audio events and loads since the last one, per the project's
    /// channels as they stand now, are held until whole (`held`). `times_gpu`
    /// says whether the adapter takes timestamps, so a frame waits on its GPU
    /// times only where they come. Call it after the frame's stats were recorded,
    /// so the newest frame of the stats is this one.
    pub fn frame(&mut self, game: &GameWorld, frame: &WindowFrame, times_gpu: bool) {
        let channels = game.resources.application.borrow().log_settings().channels;
        let place = OnceCell::new();
        let place = || place.get_or_init(|| record::Place::of(game, now_ms()));
        let mut record = None;
        let mut rest = Vec::new();
        if channels.frames || channels.loads {
            let last = game.script_manager().stats_cell().borrow().last_frame();
            if channels.frames {
                record = Some(record::frame(place().clone(), frame, &last, channels));
            }
            if channels.loads {
                rest.extend(record::load(place(), &last));
            }
        }
        let lost = self.console(game, channels.console, place, &mut rest);
        let audio = game.resources.audio.borrow();
        let (events, _) = audio.events_after(self.audio_seq);
        if let Some(newest) = events.last() {
            self.audio_seq = newest.seq;
            if channels.audio {
                rest.extend(events.iter().filter_map(|e| record::audio(place(), e)));
            }
        }
        let tick = game.play_frame();
        self.writer.lost(lost, tick);
        let entry = held::Entry::new(record, times_gpu, rest, tick);
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.ended(frame.last_wall_ms);
        if let Some(gpu) = frame.render.as_ref().and_then(|r| r.gpu.clone()) {
            held.gpu(gpu);
        }
        for (lines, tick) in held.push(entry) {
            self.writer.send(lines, tick);
        }
    }

    /// The console's lines since the last frame into `out` (when `on`), every one
    /// as it came ([`FileFeed`](rusty_script::scripting::FileFeed)); returns how
    /// many were lost before they could be written.
    fn console<'p>(
        &mut self,
        game: &GameWorld,
        on: bool,
        place: impl Fn() -> &'p record::Place,
        out: &mut Vec<String>,
    ) -> u64 {
        let mut console = game.console().borrow_mut();
        let mut lines = Vec::new();
        let mut lost = 0;
        if !std::mem::replace(&mut self.fed, true) {
            let kept: Vec<_> = console.since(0).cloned().collect();
            lost += console.logged() - kept.len() as u64;
            lines.extend(kept.into_iter().map(|(msg, level)| FileLine {
                msg,
                level,
                times: 1,
            }));
        }
        console.feed_file(true);
        let feed = console.take_file_feed();
        lines.extend(feed.lines);
        lost += feed.lost;
        drop(console);
        if !on {
            return 0;
        }
        out.extend(lines.iter().map(|line| record::console(place(), line)));
        lost
    }
}

impl Drop for FrameLog {
    /// Write what is held, as it stands, before the writer closes.
    fn drop(&mut self) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        for (lines, tick) in held.drain() {
            self.writer.send(lines, tick);
        }
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
