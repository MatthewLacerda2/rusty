use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;
use std::time::Duration;

use super::Budget;

const TIMEOUT: Duration = Duration::from_secs(30);

fn unchecked() -> Option<std::thread::ThreadId> {
    None
}

#[test]
fn a_permit_frees_its_slot_when_dropped() {
    static B: Budget = Budget::new(2, TIMEOUT, unchecked);
    assert_eq!(B.live(), 0);
    let first = B.acquire();
    assert_eq!(B.live(), 1);
    let second = B.acquire();
    assert_eq!(B.live(), 2, "both slots held");
    drop(first);
    assert_eq!(B.live(), 1, "dropping releases exactly one slot");
    drop(second);
    assert_eq!(B.live(), 0, "and the budget returns to empty");
}

#[test]
fn concurrency_never_exceeds_the_cap() {
    static B: Budget = Budget::new(2, TIMEOUT, unchecked);
    let live = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let start = Barrier::new(8);

    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                start.wait();
                let permit = B.acquire();
                let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
                live.fetch_sub(1, Ordering::SeqCst);
                drop(permit);
            });
        }
    });

    assert!(
        peak.load(Ordering::SeqCst) <= 2,
        "8 threads peaked at {} concurrent permits against a cap of 2",
        peak.load(Ordering::SeqCst)
    );
    assert_eq!(B.live(), 0, "every permit was returned");
}

#[test]
fn a_full_budget_blocks_until_a_slot_frees() {
    static B: Budget = Budget::new(1, TIMEOUT, unchecked);
    let held = B.acquire();
    let entered = Barrier::new(2);
    let acquired = AtomicUsize::new(0);

    std::thread::scope(|s| {
        s.spawn(|| {
            entered.wait();
            let _permit = B.acquire();
            acquired.store(1, Ordering::SeqCst);
        });

        entered.wait();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            acquired.load(Ordering::SeqCst),
            0,
            "the waiter took a slot while the budget was full"
        );
        drop(held);
    });

    assert_eq!(
        acquired.load(Ordering::SeqCst),
        1,
        "the waiter was let through"
    );
    assert_eq!(B.live(), 0);
}
