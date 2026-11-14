//! The warnings a crate's own tests read back: a `log` logger keeping every
//! `warn!` / `error!` record's level and text, per thread, for [`take`] and
//! [`take_leveled`].
//!
//! A moved crate's tests cannot reach the root's `logging` (it sits above every
//! member), so a test asserting "this warns once" installs this instead. It keeps
//! what the root's console copy keeps, minus its policy: every target, no repeat
//! counting, no `core::quiet` stretch; that policy is `logging`'s, tested there.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::{Mutex, Once};
use std::thread::ThreadId;

static KEPT: Mutex<Vec<(ThreadId, Level, String)>> = Mutex::new(Vec::new());

struct Keep;

impl Log for Keep {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Warn
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
            kept.push((
                std::thread::current().id(),
                record.level(),
                record.args().to_string(),
            ));
        }
    }

    fn flush(&self) {}
}

/// Install the logger for this process; a second call is a no-op.
pub fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if log::set_logger(&Keep).is_ok() {
            log::set_max_level(LevelFilter::Warn);
        }
    });
}

/// The warnings and errors this thread logged since the last `take`, oldest first.
/// Per thread, so tests sharing a process never read each other's.
pub fn take() -> Vec<String> {
    take_leveled().into_iter().map(|(_, text)| text).collect()
}

/// [`take`] with each record's level, for a test asserting a failure is an error
/// rather than a warning (what the console shows it as).
pub fn take_leveled() -> Vec<(Level, String)> {
    let me = std::thread::current().id();
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(&mut *kept)
        .into_iter()
        .partition(|(origin, _, _)| *origin == me);
    *kept = others;
    mine.into_iter()
        .map(|(_, level, text)| (level, text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_warning_is_taken_once_and_info_never() {
        init();
        log::info!("progress");
        log::warn!("a skip");
        assert_eq!(take(), ["a skip"]);
        assert!(take().is_empty());
    }

    #[test]
    fn a_record_keeps_its_level() {
        init();
        log::warn!("a skip");
        log::error!("a failure");
        let said = take_leveled();
        assert_eq!(
            said,
            [
                (Level::Warn, "a skip".to_string()),
                (Level::Error, "a failure".to_string())
            ]
        );
    }

    #[test]
    fn another_threads_warning_is_not_taken() {
        init();
        std::thread::spawn(|| log::warn!("elsewhere"))
            .join()
            .unwrap();
        assert!(take().is_empty());
    }
}
