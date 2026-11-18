use super::{GpuTimes, Pair};
use crate::timing::GpuPass;

/// Ticks of 1 ms each.
const MS: f64 = 1e6;

#[test]
fn a_pair_spread_over_several_passes_is_one_entry_charged_once() {
    // Hi-Z's one pair opens on its select (0) and closes on its compaction (6):
    // whatever ran between, 6 ms, once; the prepass after it is its own.
    let pairs = [GpuPass::Occlusion, GpuPass::SsaoPrepass].map(Pair::from);
    let times = GpuTimes::from_ticks(&[0, 6, 6, 8], &pairs, MS);
    let want = [(GpuPass::Occlusion, 6.0), (GpuPass::SsaoPrepass, 2.0)];
    assert_eq!(times.passes, want);
    assert_eq!(times.span_ms, 8.0);
    assert_eq!(times.other_ms(), 0.0);
}

#[test]
fn the_span_less_the_rows_is_what_no_pair_holds() {
    // Forward 0..4, an untimed copy 4..5, the trace 5..7, the resolve 7..8.
    let pairs = [GpuPass::Forward, GpuPass::Ssr, GpuPass::SsrResolve].map(Pair::from);
    let times = GpuTimes::from_ticks(&[0, 4, 5, 7, 7, 8], &pairs, MS);
    assert_eq!(times.total_ms(), 7.0);
    assert_eq!(times.span_ms, 8.0);
    assert_eq!(times.other_ms(), 1.0, "the copy is in no row");
}

#[test]
fn a_pair_holding_the_gap_takes_the_untimed_copy_before_it() {
    // The same frame with the trace's pair holding the gap: the copy is SSR's.
    let trace = Pair {
        pass: GpuPass::Ssr,
        holds_gap: true,
    };
    let pairs = [
        Pair::from(GpuPass::Forward),
        trace,
        GpuPass::SsrResolve.into(),
    ];
    let times = GpuTimes::from_ticks(&[0, 4, 5, 7, 7, 8], &pairs, MS);
    let want = [
        (GpuPass::Forward, 4.0),
        (GpuPass::Ssr, 3.0),
        (GpuPass::SsrResolve, 1.0),
    ];
    assert_eq!(times.passes, want);
    assert_eq!(times.other_ms(), 0.0);
}

#[test]
fn a_gap_holder_still_never_charges_work_ahead_of_it_twice() {
    // Overlap: the trace starts at 2, inside forward's 0..4; holding the gap opens
    // it at 4, as the exclusive rule does. First in a frame, it opens at its start.
    let trace = Pair {
        pass: GpuPass::Ssr,
        holds_gap: true,
    };
    let times = GpuTimes::from_ticks(&[0, 4, 2, 6], &[GpuPass::Forward.into(), trace], MS);
    assert_eq!(times.passes, [(GpuPass::Forward, 4.0), (GpuPass::Ssr, 2.0)]);
    let alone = GpuTimes::from_ticks(&[3, 6], &[trace], MS);
    assert_eq!(alone.passes, [(GpuPass::Ssr, 3.0)]);
}

#[test]
fn overlapping_pairs_charge_each_tick_once_and_the_span_counts_from_the_earliest() {
    // Post-FX starts at 1, inside forward's 2..10, and ends at 11: it costs 1, and
    // the span opens at its start, the earliest written.
    let pairs = [GpuPass::Forward, GpuPass::PostFx].map(Pair::from);
    let times = GpuTimes::from_ticks(&[2, 10, 1, 11], &pairs, MS);
    assert_eq!(times.total_ms(), 9.0);
    assert_eq!(times.span_ms, 10.0);
    assert_eq!(times.other_ms(), 1.0);
}

#[test]
fn no_pairs_no_span() {
    let times = GpuTimes::from_ticks(&[], &[], MS);
    assert!(times.passes.is_empty());
    assert_eq!((times.span_ms, times.other_ms()), (0.0, 0.0));
}
