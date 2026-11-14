//! Tests for the budget's nested-acquire check and its pairs (#1844).
//!
//! Each drives a private `Budget` whose checked holder is every thread, so a test
//! thread here stands for any test's. Adapter-free: pure synchronization.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::ThreadId;
use std::time::Duration;

use super::Budget;

/// Long, so a test that wrongly waits fails by hanging, not by a timeout panic
/// that `should_panic` would take for the nested-acquire one.
const TIMEOUT: Duration = Duration::from_secs(600);

fn every_thread() -> Option<ThreadId> {
    Some(std::thread::current().id())
}

fn caught(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
    let err = std::panic::catch_unwind(f).expect_err("it should panic");
    let literal = err.downcast_ref::<&str>().map(|s| (*s).to_owned());
    literal
        .or_else(|| err.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

#[test]
fn a_second_permit_on_the_holding_thread_panics_at_once_naming_the_pair() {
    static B: Budget = Budget::new(2, TIMEOUT, every_thread);
    let first = B.acquire();
    let message = caught(|| drop(B.acquire()));
    assert!(message.contains("headless_pair_or_skip"), "{message}");
    assert_eq!(B.live(), 1, "the refused acquire took no slot");
    drop(first);
    drop(B.acquire()); // free again: one permit at a time is fine
    assert_eq!(B.live(), 0);
}

#[test]
fn a_pair_beside_a_held_permit_panics_too() {
    static B: Budget = Budget::new(2, TIMEOUT, every_thread);
    let _first = B.acquire();
    let message = caught(|| drop(B.acquire_pair()));
    assert!(message.contains("headless_pair_or_skip"), "{message}");
}

/// A permit dropped on another thread clears its own holder's record, not the
/// dropping thread's, so the holder may take one again.
#[test]
fn a_permit_dropped_on_another_thread_frees_its_holder() {
    static B: Budget = Budget::new(2, TIMEOUT, every_thread);
    let permit = B.acquire();
    std::thread::spawn(move || drop(permit)).join().unwrap();
    let again = B.acquire();
    assert_eq!(B.live(), 1);
    drop(again);
}

/// Two threads each holding one slot never wait on each other for a pair's
/// second: the pair waits until both are free, then takes both.
#[test]
fn a_pair_takes_both_slots_at_once_and_returns_them_with_its_last_share() {
    static B: Budget = Budget::new(2, TIMEOUT, every_thread);
    let pair = B.acquire_pair();
    assert_eq!(B.live(), 2);
    let kept = pair.permit();
    let fresh = pair.permit();
    let message = caught(std::panic::AssertUnwindSafe(|| drop(pair.permit())));
    assert!(message.contains("third"), "{message}");
    drop(fresh);
    drop(pair.permit()); // the slot a dropped share left is reusable
    drop(pair);
    assert_eq!(B.live(), 2, "a live share keeps the pair's slots");
    drop(kept);
    assert_eq!(B.live(), 0);
}

#[test]
fn a_pair_waits_for_two_free_slots_without_holding_one() {
    static B: Budget = Budget::new(2, TIMEOUT, every_thread);
    let other = B.acquire(); // another test's slot, on this thread for brevity
    let got = AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            let _pair = B.acquire_pair();
            got.store(true, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(!got.load(Ordering::SeqCst), "a pair took a slot in use");
        assert_eq!(B.live(), 1, "the waiting pair holds nothing");
        drop(other);
    });
    assert!(got.load(Ordering::SeqCst));
    assert_eq!(B.live(), 0);
}

/// While a test thread holds a pair, a renderer the engine builds on it (a bake
/// beside a capture host) takes the pair's share instead of a third slot.
#[test]
fn a_single_acquire_on_a_pair_holding_thread_takes_a_share() {
    static B: Budget = Budget::new(2, TIMEOUT, every_thread);
    let pair = B.acquire_pair();
    let host = B.acquire();
    let bake = B.acquire();
    assert_eq!(B.live(), 2, "both from the pair");
    let message = caught(|| drop(B.acquire()));
    assert!(message.contains("third"), "{message}");
    drop((host, bake, pair));
    assert_eq!(B.live(), 0);
}
