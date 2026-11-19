//! A stage timed in parts (#2401): [`GpuPart`]'s pairs, summed into their
//! kind's row and their own.

use super::{GpuPart, GpuPass, GpuTimes, Pair};

/// A stage timed in parts (#2401), then a forward pass, at 1 ms a tick.
fn parts_over(parts: &[GpuPart], ticks: &[u64]) -> GpuTimes {
    let part = |p: &GpuPart| Pair {
        part: Some(*p),
        ..Pair::from(p.pass())
    };
    let pairs: Vec<_> = parts
        .iter()
        .map(part)
        .chain([GpuPass::Forward.into()])
        .collect();
    GpuTimes::from_ticks(ticks, &pairs, 1e6)
}

/// The parts that take a pair: every part but one its sub-parts sum to.
fn paired() -> Vec<GpuPart> {
    let has_subs = |p: &GpuPart| GpuPart::ALL.iter().any(|s| s.within() == Some(*p));
    GpuPart::ALL.into_iter().filter(|p| !has_subs(p)).collect()
}

#[test]
fn the_occlusion_parts_sum_to_its_row_and_a_missing_one_is_absent() {
    // Select 1..2, phase one 2..5, pyramid 4..6 (charged from 5), test 6..7,
    // the cluster cull's test 7..8, scan 8..9 and copy 9..10, then forward 10..11.
    let ticks = [1, 2, 2, 5, 4, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11];
    let times = parts_over(&paired(), &ticks);
    let ms: Vec<_> = times.parts.iter().map(|(_, ms)| *ms).collect();
    assert_eq!(
        ms,
        [1.0, 3.0, 1.0, 1.0, 3.0, 1.0, 1.0, 1.0],
        "clusters is its three"
    );
    assert_eq!(
        times.passes,
        [(GpuPass::Occlusion, 9.0), (GpuPass::Forward, 1.0)]
    );
    assert_eq!(
        times.total_ms(),
        10.0,
        "a part is in the frame's total once"
    );
    let top = times.parts.iter().filter(|(p, _)| p.within().is_none());
    assert_eq!(
        top.map(|(_, ms)| ms).sum::<f64>(),
        9.0,
        "the parts within none sum to the row"
    );
    // Without the cluster cull its pairs are never taken: no row, not a zero.
    let times = parts_over(&paired()[..4], &[1, 2, 2, 5, 4, 6, 6, 7, 10, 11]);
    assert!(!times
        .parts
        .iter()
        .any(|(p, _)| *p == GpuPart::OcclusionClusters || p.within().is_some()));
    let sum: f64 = times.parts.iter().map(|(_, ms)| ms).sum();
    assert_eq!(times.passes[0], (GpuPass::Occlusion, sum));
}

#[test]
fn part_names_are_stable_and_in_discriminant_order() {
    let parts: Vec<_> = GpuPart::ALL.iter().map(|p| p.row()).collect();
    let names = [
        "select",
        "phase_one",
        "pyramid",
        "test",
        "clusters",
        "clusters_test",
        "clusters_scan",
        "clusters_copy",
    ];
    assert_eq!(parts, names.map(|p| format!("occlusion.{p}")));
    for (i, part) in GpuPart::ALL.iter().enumerate() {
        assert_eq!(*part as usize, i);
    }
}
