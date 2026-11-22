use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::budget::{cost, Budget, BUDGET};
use super::ffi::KEEP_TEXELS;

fn near_half() -> usize {
    let mut texels = KEEP_TEXELS + 1;
    while cost(texels * 2) * 2 < BUDGET {
        texels *= 2;
    }
    assert!(cost(texels) * 2 <= BUDGET && cost(texels) * 3 > BUDGET);
    texels
}

fn run(jobs: usize, texels: usize) -> (usize, u64) {
    let budget = Budget::default();
    let (now, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let (held, peak) = (AtomicU64::new(0), AtomicU64::new(0));
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                budget.run(texels, || {
                    most.fetch_max(now.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                    let bytes = held.fetch_add(cost(texels), Ordering::SeqCst) + cost(texels);
                    peak.fetch_max(bytes, Ordering::SeqCst);
                    for _ in 0..1000 {
                        std::thread::yield_now();
                    }
                    held.fetch_sub(cost(texels), Ordering::SeqCst);
                    now.fetch_sub(1, Ordering::SeqCst);
                })
            });
        }
    });
    (most.into_inner(), peak.into_inner())
}

#[test]
fn large_denoises_in_flight_stay_within_the_budget() {
    let (most, peak) = run(6, near_half());
    assert!(most <= 2, "{most} in flight");
    assert!(peak <= BUDGET, "{peak} bytes held");
}

#[test]
fn a_denoise_past_the_budget_still_runs_alone() {
    let mut texels = near_half();
    while cost(texels) <= BUDGET {
        texels *= 2;
    }
    let (most, _) = run(3, texels);
    assert_eq!(most, 1, "one at a time, and every one finished");
}

#[test]
fn a_small_denoise_never_waits_on_the_budget() {
    let budget = Budget::default();
    let large = near_half();
    budget.run(large, || {
        budget.run(large, || assert_eq!(budget.run(KEEP_TEXELS, || 7), 7))
    });
}
