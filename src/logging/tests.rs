use std::cell::RefCell;

use super::*;
use crate::scripting::LogLevel;

fn console() -> RefCell<ConsoleLogs> {
    RefCell::new(ConsoleLogs::new())
}

#[test]
fn an_engine_warning_lands_in_the_console() {
    init();
    log::warn!("[Test] a mesh collider was skipped");
    log::error!("[Test] a texture did not load");

    let console = console();
    let forwarded = forward(&console);
    assert!(forwarded.contains(&"[Test] a mesh collider was skipped".to_string()));
    let messages = &console.borrow().messages;
    assert!(messages.contains(&(
        "[Test] a mesh collider was skipped".to_string(),
        LogLevel::Warning
    )));
    assert!(messages.contains(&("[Test] a texture did not load".to_string(), LogLevel::Error)));
    assert!(
        forward(&console).is_empty(),
        "a drain takes each record once"
    );
}

#[test]
fn info_and_dependency_warnings_stay_out_of_the_console() {
    init();
    log::info!("[Test] routine progress");
    log::warn!(target: "wgpu_hal::vulkan", "[Dep] a driver quirk");

    let forwarded = forward(&console());
    assert!(!forwarded.iter().any(|m| m.contains("routine progress")));
    assert!(!forwarded.iter().any(|m| m.contains("driver quirk")));
}

#[test]
fn the_buffer_keeps_only_the_newest_records() {
    for n in 0..CAPACITY + 5 {
        keep(Level::Warn, format!("record {n}"));
    }
    let forwarded = forward(&console());
    assert_eq!(forwarded.len(), CAPACITY);
    assert_eq!(forwarded[0], "record 5");
}
