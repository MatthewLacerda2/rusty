use std::cell::RefCell;

use super::*;

// Every assertion looks for its own tag with `contains`: under plain `cargo test`
// other tests share the buffer (each drain takes only its own thread's records,
// `take_mine`), and the console may hold their entries.

fn console() -> RefCell<ConsoleLogs> {
    RefCell::new(ConsoleLogs::new())
}

fn has(console: &RefCell<ConsoleLogs>, text: &str, level: LogLevel) -> bool {
    let entry = (text.to_string(), level);
    console.borrow().messages.contains(&entry)
}

#[test]
fn an_engine_warning_lands_in_the_console() {
    init();
    log::warn!("[Test] lands: a mesh collider was skipped");
    log::error!("[Test] lands: a texture did not load");

    let console = console();
    let forwarded = forward(&console);
    let warning = "[Test] lands: a mesh collider was skipped";
    assert!(forwarded.iter().any(|m| m == warning));
    assert!(has(&console, warning, LogLevel::Warning));
    let error = "[Test] lands: a texture did not load";
    assert!(has(&console, error, LogLevel::Error));
    let again = forward(&console);
    assert!(
        !again.iter().any(|m| m.contains("[Test] lands:")),
        "a drain takes each record once"
    );
}

#[test]
fn info_and_dependency_warnings_stay_out_of_the_console() {
    init();
    log::info!("[Test] out: routine progress");
    log::warn!(target: "wgpu_hal::vulkan", "[Dep] out: a driver quirk");

    let forwarded = forward(&console());
    assert!(!forwarded
        .iter()
        .any(|m| m.contains("out: routine progress")));
    assert!(!forwarded.iter().any(|m| m.contains("out: a driver quirk")));
}

#[test]
fn a_warning_raised_every_frame_is_one_entry() {
    init();
    for _ in 0..250 {
        log::warn!("[Test] collapse: per-frame warning");
    }
    let console = console();
    let forwarded = forward(&console);
    let shown = "[Test] collapse: per-frame warning (×250)";
    assert!(forwarded.iter().any(|m| m == shown));
    assert!(has(&console, shown, LogLevel::Warning));
}

#[test]
fn the_buffer_keeps_only_the_newest_records() {
    // A local buffer: the shared one may hold other tests' records.
    let mut pending = Vec::new();
    for n in 0..CAPACITY + 5 {
        keep(&mut pending, Level::Warn, format!("record {n}"));
    }
    assert_eq!(pending.len(), CAPACITY);
    assert_eq!(pending[0].text, "record 5");
}
