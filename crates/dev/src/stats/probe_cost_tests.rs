//! The timing probe's end-of-frame work (its frame-end counter read and the
//! recording at the tick's end) is `probe_ms`, beside `frame_ms` and never in it:
//! a shipped game runs no probe, so a budget on the tick never reads it. The
//! driven clock (`verbs::drive`) charges both steps exactly, each its own cost.

use super::counters::{END_COST, RECORD_COST};
use super::stats_tick_tests::play;

/// What the probe's counter read charges the driven clock, in ms.
const COST: u64 = 7;
/// What the tick's end-of-tick recording charges it, in ms.
const END: u64 = 3;

/// `ticks` play ticks with the probe's counter read charged [`COST`] each and its
/// end-of-tick recording [`END`].
fn costly(name: &str, ticks: u32) -> rusty_core::frame_stats::FrameStats {
    RECORD_COST.with(|c| c.set(COST));
    END_COST.with(|c| c.set(END));
    let stats = play(name, ticks, |_| {}, |_, _| {});
    RECORD_COST.with(|c| c.set(0));
    END_COST.with(|c| c.set(0));
    stats
}

#[test]
fn the_probes_own_work_is_beside_the_tick_never_in_it() {
    let stats = costly("probe_cost", 4);
    let frame = stats.get("frame_ms").expect("ticks were timed");
    assert_eq!(
        (frame.samples, frame.max),
        (4, 0.0),
        "no probe work in the tick"
    );
    assert!(stats.check_budget("frame_ms", 1.0).is_ok());
    let probe = stats.get("probe_ms").expect("the probe's work is timed");
    assert_eq!(probe.samples, 4, "one sample a play tick");
    let both = (COST + END) as f64;
    assert_eq!(
        (probe.min, probe.max),
        (both, both),
        "the counter read and the end-of-tick recording, each tick"
    );
}

#[test]
fn a_probe_that_costs_nothing_still_reports_its_row_every_play_tick() {
    let free = play("probe_free", 2, |_| {}, |_, _| {});
    let row = free.get("probe_ms").map(|s| (s.samples, s.max));
    assert_eq!(
        row,
        Some((2, 0.0)),
        "present, so a report row never comes and goes"
    );
}
