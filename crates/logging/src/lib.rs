use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::thread::ThreadId;

use log::{Level, LevelFilter, Log, Metadata, Record};

use rusty_script::scripting::{ConsoleLogs, LogLevel};

#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod one_logger_tests;
#[cfg(test)]
mod tests;

const CAPACITY: usize = 100;

/// One kept record: the thread that logged it, and how many times in a row it came.
struct Kept {
    origin: ThreadId,
    level: Level,
    text: String,
    times: u32,
}

static PENDING: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

static PER_THREAD: AtomicBool = AtomicBool::new(false);

/// Whether [`init`] installed [`Forward`]: `log::set_logger` is once a process.
static INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let env = env_logger::Env::default().default_filter_or("warn");
        let stderr = env_logger::Builder::from_env(env).build();
        // `warn!` must stay enabled for the console copy even under `RUST_LOG=error`.
        let max = stderr.filter().max(LevelFilter::Warn);
        if log::set_boxed_logger(Box::new(Forward { stderr })).is_ok() {
            log::set_max_level(max);
            INSTALLED.store(true, Ordering::SeqCst);
        }
    });
}

#[doc(hidden)]
pub fn init_for_tests() {
    // lint: allow(test-static-write) one-way and idempotent: every caller sets the same `true`, and per-thread drains are what every test in a shared process wants
    PER_THREAD.store(true, Ordering::SeqCst);
    init();
    assert!(INSTALLED.load(Ordering::SeqCst), "{ANOTHER_LOGGER}");
}

/// What [`init_for_tests`] says when another logger holds the process.
#[doc(hidden)]
pub const ANOTHER_LOGGER: &str = "another `log` logger holds this process (most likely \
    `rusty_test_support::logs`'), so `rusty_logging::forward` would drain nothing: a \
    crate whose manifest names `rusty-logging` reads its tests' warnings through \
    `rusty_logging::init_for_tests` and `rusty_logging::forward`, and never calls \
    `rusty_test_support::logs::init`";

pub fn forward(console: &RefCell<ConsoleLogs>) -> Vec<String> {
    let records = {
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        take_mine(&mut pending, PER_THREAD.load(Ordering::SeqCst))
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

fn take_mine(pending: &mut Vec<Kept>, per_thread: bool) -> Vec<Kept> {
    let taken = if per_thread {
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
        // A staged copy's warnings are the real apply's again (`core::quiet`).
        if rusty_core::quiet::is_quiet() && is_kept(record.metadata()) {
            return;
        }
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

fn is_kept(metadata: &Metadata<'_>) -> bool {
    let krate = metadata.target().split("::").next().unwrap_or_default();
    let ours = krate == "rusty" || krate.starts_with("rusty_");
    ours && metadata.level() <= Level::Warn
}

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
