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

#[test]
fn the_occlusion_parts_sum_to_its_row_and_a_missing_one_is_absent() {
    // Select 1..2, phase one 2..5, pyramid 4..6 (charged from 5), test 6..7,
    // clusters 7..10, then forward 10..11.
    let ticks = [1, 2, 2, 5, 4, 6, 6, 7, 7, 10, 10, 11];
    let times = parts_over(&GpuPart::ALL, &ticks);
    let ms: Vec<_> = times.parts.iter().map(|(_, ms)| *ms).collect();
    assert_eq!(ms, [1.0, 3.0, 1.0, 1.0, 3.0]);
    assert_eq!(
        times.passes,
        [(GpuPass::Occlusion, 9.0), (GpuPass::Forward, 1.0)]
    );
    assert_eq!(
        times.total_ms(),
        10.0,
        "a part is in the frame's total once"
    );
    // Without the cluster cull its pair is never taken: no row, not a zero.
    let times = parts_over(&GpuPart::ALL[..4], &[1, 2, 2, 5, 4, 6, 6, 7, 10, 11]);
    assert!(!times
        .parts
        .iter()
        .any(|(p, _)| *p == GpuPart::OcclusionClusters));
    let sum: f64 = times.parts.iter().map(|(_, ms)| ms).sum();
    assert_eq!(times.passes[0], (GpuPass::Occlusion, sum));
}

#[test]
fn part_names_are_stable_and_in_discriminant_order() {
    let parts: Vec<_> = GpuPart::ALL.iter().map(|p| p.row()).collect();
    let five = ["select", "phase_one", "pyramid", "test", "clusters"];
    assert_eq!(parts, five.map(|p| format!("occlusion.{p}")));
    for (i, part) in GpuPart::ALL.iter().enumerate() {
        assert_eq!(*part as usize, i);
    }
}
