//! src/logging/ — the one logger every binary starts (#1139).
//!
//! [`init`] installs `env_logger` with a default filter of `warn` (`RUST_LOG`
//! overrides it), so an engine warning reaches stderr without anyone asking for it.
//! The logger also keeps a copy of every **engine** `warn!` / `error!` record (its
//! target is in this crate; a dependency's warnings go to stderr only) in a small
//! buffer. Records arrive from any thread (the audio device thread logs too), so the
//! buffer is a mutex, and whoever owns a console drains it once per frame or command
//! with [`forward`]: the shell's frame loop, the headless session, the harness. That
//! is how the editor console, `play`'s `console.log` and the session/MCP replies see
//! what the engine dropped, as Unity's Console shows `Debug.LogWarning`.
//!
//! A record that repeats the one before it is counted, not stored again, here and in
//! the console ([`ConsoleLogs::repeated`]): a warning raised every frame is one entry.
//!
//! Platform-side on purpose: the sim only *writes* records through the `log` macros;
//! nothing in a sim module reads this buffer, so the determinism rule is untouched.

use std::cell::RefCell;
use std::sync::{Mutex, Once};
use std::thread::ThreadId;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::scripting::{ConsoleLogs, LogLevel};

#[cfg(test)]
mod tests;

/// Records kept for the console between drains; past this the oldest are dropped,
/// so a binary that never drains (none should) holds a bounded amount.
const CAPACITY: usize = 100;

/// One kept record: the thread that logged it, and how many times in a row it came.
struct Kept {
    origin: ThreadId,
    level: Level,
    text: String,
    times: u32,
}

static PENDING: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

/// Start logging for this process: stderr at `RUST_LOG` (default `warn`), plus the
/// console copy of engine warnings and errors. Every binary calls it first thing; a
/// second call is a no-op.
pub fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let env = env_logger::Env::default().default_filter_or("warn");
        let stderr = env_logger::Builder::from_env(env).build();
        // `warn!` must stay enabled for the console copy even under `RUST_LOG=error`.
        let max = stderr.filter().max(LevelFilter::Warn);
        if log::set_boxed_logger(Box::new(Forward { stderr })).is_ok() {
            log::set_max_level(max);
        }
    });
}

/// Move the engine records logged since the last drain into `console` (as warnings
/// or errors) and return their text, oldest first, a repeat shown with its count.
pub fn forward(console: &RefCell<ConsoleLogs>) -> Vec<String> {
    let records = {
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        take_mine(&mut pending)
    };
    if records.is_empty() {
        return Vec::new();
    }
    let mut console = console.borrow_mut();
    records
        .into_iter()
        .map(|kept| {
            let level = match kept.level {
                Level::Error => LogLevel::Error,
                _ => LogLevel::Warning,
            };
            let shown = ConsoleLogs::collapsed(&kept.text, kept.times);
            console.repeated(kept.text, level, kept.times);
            shown
        })
        .collect()
}

/// The records a drain takes: all of them. Under `cargo test` every test is a thread
/// sharing this buffer, so a test takes only what its own thread logged, and one
/// test's drain never steals another's warnings (nextest runs a process per test).
/// Repeats left side by side once another thread's records are set aside count as
/// one run.
fn take_mine(pending: &mut Vec<Kept>) -> Vec<Kept> {
    let taken = if cfg!(test) {
        let me = std::thread::current().id();
        let (mine, others) = std::mem::take(pending)
            .into_iter()
            .partition(|kept| kept.origin == me);
        *pending = others;
        mine
    } else {
        std::mem::take(pending)
    };
    let mut runs: Vec<Kept> = Vec::with_capacity(taken.len());
    for kept in taken {
        match runs.last_mut() {
            Some(run) if run.level == kept.level && run.text == kept.text => {
                run.times += kept.times;
            }
            _ => runs.push(kept),
        }
    }
    runs
}

/// `env_logger` for stderr, with a copy of engine warnings kept for [`forward`].
struct Forward {
    stderr: env_logger::Logger,
}

impl Log for Forward {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        is_kept(metadata) || self.stderr.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if self.stderr.matches(record) {
            self.stderr.log(record);
        }
        if is_kept(record.metadata()) {
            let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
            keep(&mut pending, record.level(), record.args().to_string());
        }
    }

    fn flush(&self) {
        self.stderr.flush();
    }
}

/// A warning or error raised by this crate's own code.
fn is_kept(metadata: &Metadata<'_>) -> bool {
    let target = metadata.target();
    let ours = target == "rusty" || target.starts_with("rusty::");
    ours && metadata.level() <= Level::Warn
}

/// Add one record to `pending`: a repeat of the newest record counts up, anything
/// else is appended, dropping the oldest past [`CAPACITY`].
fn keep(pending: &mut Vec<Kept>, level: Level, text: String) {
    let origin = std::thread::current().id();
    let newest = pending.last_mut();
    if let Some(kept) = newest.filter(|k| k.origin == origin && k.level == level && k.text == text)
    {
        kept.times += 1;
        return;
    }
    if pending.len() >= CAPACITY {
        pending.remove(0);
    }
    pending.push(Kept {
        origin,
        level,
        text,
        times: 1,
    });
}
