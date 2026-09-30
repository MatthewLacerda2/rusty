//! Tests for the sidecar rename retry (#520). Windows can't run here, so the retry
//! decision and loop are exercised by injecting the errors Windows would return.

use super::{is_transient, retry};
use std::io::{self, ErrorKind};
use std::time::Duration;

fn denied() -> io::Error {
    io::Error::from(ErrorKind::PermissionDenied)
}

#[test]
fn windows_treats_denied_and_sharing_violations_as_transient() {
    assert!(is_transient(&denied(), true));
    assert!(is_transient(&io::Error::from_raw_os_error(32), true));
    assert!(is_transient(&io::Error::from_raw_os_error(33), true));
}

#[test]
fn other_errors_are_never_transient() {
    assert!(!is_transient(&io::Error::from(ErrorKind::NotFound), true));
    assert!(!is_transient(&io::Error::other("disk on fire"), true));
}

#[test]
fn unix_never_retries_even_a_permission_error() {
    // On Unix a rename replaces an open file, so `PermissionDenied` is a real
    // permissions problem and retrying would only delay the error.
    assert!(!is_transient(&denied(), false));
    assert!(!is_transient(&io::Error::from_raw_os_error(32), false));
}

/// Drive `retry` with scripted outcomes; returns its result and how many calls ran.
fn run(script: Vec<io::Result<u8>>, attempts: u32) -> (io::Result<u8>, usize) {
    let mut outcomes = script.into_iter();
    let mut calls = 0;
    let result = retry(
        attempts,
        Duration::ZERO,
        |e| is_transient(e, true),
        || {
            calls += 1;
            outcomes.next().expect("retry ran past the script")
        },
    );
    (result, calls)
}

#[test]
fn a_transient_failure_that_clears_succeeds() {
    let (result, calls) = run(vec![Err(denied()), Err(denied()), Ok(7)], 5);
    assert_eq!(result.unwrap(), 7);
    assert_eq!(calls, 3);
}

#[test]
fn a_permanent_failure_is_reported_at_once() {
    let (result, calls) = run(vec![Err(io::Error::from(ErrorKind::NotFound))], 5);
    assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(calls, 1);
}

#[test]
fn retries_are_bounded_and_report_the_last_error() {
    let (result, calls) = run((0..3).map(|_| Err(denied())).collect(), 3);
    assert_eq!(result.unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(calls, 3);
}
