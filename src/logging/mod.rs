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
//! Platform-side on purpose: the sim only *writes* records through the `log` macros;
//! nothing in a sim module reads this buffer, so the determinism rule is untouched.

use std::cell::RefCell;
use std::sync::{Mutex, Once};

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::scripting::ConsoleLogs;

#[cfg(test)]
mod tests;

/// Records kept for the console between drains; past this the oldest are dropped,
/// so a binary that never drains (none should) holds a bounded amount.
const CAPACITY: usize = 100;

static PENDING: Mutex<Vec<(Level, String)>> = Mutex::new(Vec::new());

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
/// or errors) and return their text, oldest first.
pub fn forward(console: &RefCell<ConsoleLogs>) -> Vec<String> {
    let records = std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
    if records.is_empty() {
        return Vec::new();
    }
    let mut console = console.borrow_mut();
    records
        .into_iter()
        .map(|(level, text)| {
            match level {
                Level::Error => console.error(text.clone()),
                _ => console.warn(text.clone()),
            }
            text
        })
        .collect()
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
            keep(record.level(), record.args().to_string());
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

fn keep(level: Level, text: String) {
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pending.len() >= CAPACITY {
        pending.remove(0);
    }
    pending.push((level, text));
}
