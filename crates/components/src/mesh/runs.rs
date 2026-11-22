use std::ops::Range;

pub fn material_runs(runs: Vec<Range<u32>>, slots: Option<usize>) -> Vec<(usize, Range<u32>)> {
    let Some(slots) = slots else {
        return runs.into_iter().enumerate().collect();
    };
    let Some(last) = runs.last() else {
        return Vec::new();
    };
    (0..slots)
        .map(|slot| (slot, runs.get(slot).unwrap_or(last).clone()))
        .collect()
}

pub fn clip_runs(runs: impl Iterator<Item = Range<u32>>, count: u32) -> Vec<Range<u32>> {
    runs.map(|r| r.start.min(count)..r.end.min(count))
        .filter(|r| !r.is_empty())
        .collect()
}
