use super::{GpuTimes, Pair};
use crate::timing::GpuPass;

/// Ticks of 1 ms each.
const MS: f64 = 1e6;

/// A pair of `pass` that went out in submit `submit`.
fn sent(pass: GpuPass, submit: u64) -> Pair {
    Pair { pass, submit }
}

#[test]
fn a_pair_spread_over_several_passes_is_one_entry_charged_once() {
    // Hi-Z's one pair opens on its select (1) and closes on its compaction (7):
    // whatever ran between, 6 ms, once; the prepass after it is its own.
    let pairs = [GpuPass::Occlusion, GpuPass::SsaoPrepass].map(Pair::from);
    let times = GpuTimes::from_ticks(&[1, 7, 7, 9], &pairs, MS);
    let want = [(GpuPass::Occlusion, 6.0), (GpuPass::SsaoPrepass, 2.0)];
    assert_eq!(times.passes, want);
    assert_eq!(times.span_ms, 8.0);
    assert_eq!(times.other_ms(), 0.0);
}

#[test]
fn a_gap_inside_one_submit_is_untimed_work() {
    // Forward 1..5, an untimed command 5..6, post-FX 6..8, one command buffer.
    let pairs = [GpuPass::Forward, GpuPass::PostFx].map(|p| sent(p, 3));
    let times = GpuTimes::from_ticks(&[1, 5, 6, 8], &pairs, MS);
    assert_eq!(times.total_ms(), 6.0);
    assert_eq!(times.span_ms, 7.0);
    assert_eq!(times.other_ms(), 1.0, "the command is in no row");
    assert_eq!(times.between_submits_ms, 0.0);
    assert_eq!(times.within_submits_ms(), 1.0);
}

#[test]
fn a_gap_from_one_submit_to_the_next_is_told_apart_and_charged_to_no_row() {
    // Forward 1..5 in submit 1; SSR's copy and a wait 5..7 head submit 2, whose
    // trace runs 7..9 and resolve, after an untimed 9..10, 10..11.
    let pairs = [
        sent(GpuPass::Forward, 1),
        sent(GpuPass::Ssr, 2),
        sent(GpuPass::SsrResolve, 2),
    ];
    let times = GpuTimes::from_ticks(&[1, 5, 7, 9, 10, 11], &pairs, MS);
    let want = [
        (GpuPass::Forward, 4.0),
        (GpuPass::Ssr, 2.0),
        (GpuPass::SsrResolve, 1.0),
    ];
    assert_eq!(times.passes, want, "no row takes a gap");
    assert_eq!(times.other_ms(), 3.0);
    assert_eq!(times.between_submits_ms, 2.0);
    assert_eq!(times.within_submits_ms(), 1.0);
}

#[test]
fn overlapping_pairs_charge_each_tick_once_and_the_span_counts_from_the_earliest() {
    // Post-FX starts at 1, inside forward's 2..10, and ends at 11: it costs 1, and
    // the span opens at its start, the earliest written. Overlap is no gap.
    let pairs = [sent(GpuPass::Forward, 1), sent(GpuPass::PostFx, 2)];
    let times = GpuTimes::from_ticks(&[2, 10, 1, 11], &pairs, MS);
    assert_eq!(times.total_ms(), 9.0);
    assert_eq!(times.span_ms, 10.0);
    assert_eq!(times.other_ms(), 1.0);
    assert_eq!(times.between_submits_ms, 0.0);
}

#[test]
fn a_pair_not_written_whole_stretches_neither_the_span_nor_the_gaps() {
    // A pair taken and never written reads a start of 0 (or one past its end):
    // counted, the span would run from tick 0, hours on a real clock.
    let at = 3_600_000; // an hour in, in ms ticks
    for ends in [[0, at + 2], [at + 9, at + 2]] {
        let ticks = [at, at + 2, ends[0], ends[1], at + 2, at + 4];
        let pairs = [
            sent(GpuPass::Forward, 1),
            sent(GpuPass::Ui, 1),
            sent(GpuPass::PostFx, 2),
        ];
        let times = GpuTimes::from_ticks(&ticks, &pairs, MS);
        assert_eq!(times.span_ms, 4.0, "{ends:?}");
        assert_eq!(times.other_ms(), 0.0, "{ends:?}");
        assert_eq!(times.between_submits_ms, 0.0, "{ends:?}");
    }
}

#[test]
fn no_pairs_no_span() {
    let times = GpuTimes::from_ticks(&[], &[], MS);
    assert!(times.passes.is_empty());
    assert_eq!((times.span_ms, times.other_ms()), (0.0, 0.0));
}
