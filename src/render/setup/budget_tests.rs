//! Tests for the headless-renderer budget (#366).
//!
//! Every test drives its **own** `Budget` static, never the process-wide `HEADLESS`
//! one — the real GPU tests hold permits on that while these run beside them, and
//! sharing a counter with them would make any exact assertion race.
//!
//! Adapter-free by construction: the guard is pure synchronization, so these run on
//! every platform including the Linux CI job where GPU tests skip entirely.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;
use std::time::Duration;

use super::Budget;

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn a_permit_frees_its_slot_when_dropped() {
    static B: Budget = Budget::new(2, TIMEOUT);
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

/// The invariant the whole issue exists for: however many threads pile in, the number
/// alive at once never exceeds the cap. Fails against an unguarded constructor, which
/// is the state #365 tripped over on Windows.
#[test]
fn concurrency_never_exceeds_the_cap() {
    static B: Budget = Budget::new(2, TIMEOUT);
    let live = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let start = Barrier::new(8);

    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                start.wait(); // maximize the overlap: all 8 rush the budget at once
                let permit = B.acquire();
                let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                // Hold it long enough that an unbounded budget would visibly stack.
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

/// A full budget genuinely blocks rather than over-issuing: the waiter only proceeds
/// once a holder drops. Pins that `acquire` waits on the condvar instead of, say,
/// returning an already-saturated permit.
#[test]
fn a_full_budget_blocks_until_a_slot_frees() {
    static B: Budget = Budget::new(1, TIMEOUT);
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
        // The waiter is now contending for the one held slot. It cannot have taken it.
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            acquired.load(Ordering::SeqCst),
            0,
            "the waiter took a slot while the budget was full"
        );
        drop(held); // releasing lets it through, and the scope join proves it did
    });

    assert_eq!(
        acquired.load(Ordering::SeqCst),
        1,
        "the waiter was let through"
    );
    assert_eq!(B.live(), 0);
}
