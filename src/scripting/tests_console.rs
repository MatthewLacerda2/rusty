use super::console::{ConsoleLogs, LogLevel};

#[test]
fn all_three_severity_methods_produce_correct_level() {
    let mut c = ConsoleLogs::new();
    c.info("info".to_string());
    c.warn("warn".to_string());
    c.error("err".to_string());
    assert_eq!(c.messages.len(), 3);
    assert_eq!(c.messages[0], ("info".to_string(), LogLevel::Info));
    assert_eq!(c.messages[1], ("warn".to_string(), LogLevel::Warning));
    assert_eq!(c.messages[2], ("err".to_string(), LogLevel::Error));
}

#[test]
fn buffer_cap_evicts_oldest_message() {
    let mut c = ConsoleLogs::new();
    for i in 0..=100u32 {
        c.info(i.to_string());
    }
    assert_eq!(c.messages.len(), 100, "capped at 100");
    assert_eq!(c.messages[0].0, "1", "message '0' was evicted");
    assert_eq!(c.messages[99].0, "100", "last message is '100'");
}
