//! One logger a process: `log::set_logger` takes the first and refuses the rest,
//! so a test binary that installs both this crate's logger and
//! `rusty_test_support::logs`' would capture nothing through whichever came
//! second. Each says so instead, loudly. Each direction installs a logger for
//! good, so it runs in a process of its own (`rusty_test_support::alone!`).

use std::panic::catch_unwind;

use rusty_test_support::logs;

/// The message `second` panicked with, after `first` installed its logger.
fn refusal(first: fn(), second: fn()) -> String {
    first();
    let err = catch_unwind(second).expect_err("the second logger refuses");
    let text = err.downcast_ref::<&str>().map(|s| s.to_string());
    text.or_else(|| err.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

#[test]
fn the_engine_logger_refuses_when_test_support_s_came_first() {
    rusty_test_support::alone!(|| {
        let said = refusal(logs::init, super::init_for_tests);
        assert!(said.contains("rusty_test_support::logs"), "{said}");
        assert!(said.contains("rusty_logging::forward"), "{said}");
    });
}

#[test]
fn test_support_s_logger_refuses_when_the_engine_s_came_first() {
    rusty_test_support::alone!(|| {
        let said = refusal(super::init_for_tests, logs::init);
        assert_eq!(said, logs::ANOTHER_LOGGER);
        assert!(said.contains("rusty_logging::init_for_tests"), "{said}");
    });
}
