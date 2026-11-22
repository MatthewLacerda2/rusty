//! The rolling frame and console log on disk (`docs/api/Debug.md`, *The frame log*). The
//! wall clock is written here, display-side: nothing the sim reads changes.
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

pub const LOGS_DIR: &str = "saved/logs";

/// Milliseconds of CPU over a frame's ticks a system must reach to be written: the ones
/// worth a look, not the dozens that cost nothing.
pub const SYSTEM_MS: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogHost {
    Editor,
    Player,
}

pub struct FrameLog {
    writer: writer::Writer,
    held: held::Shared,
    /// Until the console feeds this log, what it logged before the first frame (a boot
    /// scene's script error) comes from its buffer.
    fed: bool,
    audio_seq: u64,
}

impl FrameLog {
    pub fn for_host(host: LogHost, settings: LogSettings) -> Option<Self> {
        let on = match host {
            LogHost::Editor => settings.editor,
            LogHost::Player => settings.player,
        };
        on.then(|| Self::start(Path::new(LOGS_DIR), settings))
            .flatten()
    }

    /// Public as a test hook too: other crates' tests start a log in a temp folder.
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

    /// Call after the frame's stats were recorded, so their newest frame is this one.
    /// `times_gpu`: the adapter takes timestamps, so a frame waits on GPU times.
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
    fn drop(&mut self) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        for (lines, tick) in held.drain() {
            self.writer.send(lines, tick);
        }
    }
}

pub fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "tests.rs"]
mod frame_log_tests;
