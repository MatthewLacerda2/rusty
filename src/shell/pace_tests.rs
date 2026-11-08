//! When the window draws: on demand outside Play, flat out otherwise (#1721).

use super::*;

const ASLEEP: Pace = Pace::OnDemand { next: None };

/// A pacer with every frame owed to its last trigger drawn.
fn settled() -> Pacer {
    let mut pacer = Pacer::default();
    for _ in 0..SETTLE_FRAMES {
        pacer.drew();
    }
    pacer
}

#[test]
fn a_new_window_draws_then_sleeps() {
    let now = Instant::now();
    let mut pacer = Pacer::default();
    assert_eq!(pacer.next(ASLEEP, now), Next::Now);
    for _ in 0..SETTLE_FRAMES {
        assert_eq!(pacer.next(ASLEEP, now), Next::Now);
        pacer.drew();
    }
    assert_eq!(pacer.next(ASLEEP, now), Next::Idle);
}

#[test]
fn activity_draws_again_after_sleeping() {
    let now = Instant::now();
    let mut pacer = settled();
    pacer.activity();
    assert_eq!(pacer.next(ASLEEP, now), Next::Now);
}

#[test]
fn a_running_game_always_draws() {
    let now = Instant::now();
    assert_eq!(settled().next(Pace::Continuous, now), Next::Now);
    let mut refused = settled();
    refused.skipped(now);
    assert_eq!(refused.next(Pace::Continuous, now), Next::Now);
}

#[test]
fn a_requested_deadline_sleeps_until_it_and_then_draws() {
    let now = Instant::now();
    let at = now + Duration::from_millis(500);
    let pace = Pace::OnDemand { next: Some(at) };
    assert_eq!(settled().next(pace, now), Next::At(at));
    assert_eq!(settled().next(pace, at), Next::Now);
}

#[test]
fn a_refused_frame_is_retried_later_not_in_a_spin_and_not_forgotten() {
    let now = Instant::now();
    let mut pacer = settled();
    pacer.activity();
    pacer.skipped(now);
    // Even an overdue egui deadline waits: the swapchain just said no.
    let overdue = Pace::OnDemand { next: Some(now) };
    assert_eq!(pacer.next(overdue, now), Next::At(now + RETRY_AFTER));
    assert_eq!(pacer.next(ASLEEP, now + RETRY_AFTER), Next::Now);
    pacer.drew();
    assert_eq!(
        pacer.next(ASLEEP, now + RETRY_AFTER),
        Next::Now,
        "still owed"
    );
}

#[test]
fn time_asleep_is_not_a_step_while_a_running_game_keeps_its_real_one() {
    assert_eq!(ASLEEP.frame_step(3600.0), MAXIMUM_DELTA_TIME);
    assert_eq!(ASLEEP.frame_step(0.016), 0.016);
    assert_eq!(Pace::Continuous.frame_step(3600.0), 3600.0);
}
