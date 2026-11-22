use std::collections::BTreeMap;

use rusty_scene::lighting::occlusion::Grid;

pub(super) fn fold_rows(
    grid: &Grid,
    seen: &[Option<Vec<u64>>],
    objects: usize,
) -> (Vec<u64>, Vec<u32>) {
    let words = objects.div_ceil(64);
    let every = every_object(objects);
    let mut ids: BTreeMap<Vec<u64>, u32> = BTreeMap::new();
    let (mut rows, mut cells) = (Vec::new(), Vec::with_capacity(seen.len()));
    let mut bits = vec![0u64; words];
    for cell in 0..seen.len() {
        bits.fill(0);
        let sees_all = grid.neighbourhood(cell).any(|n| match &seen[n] {
            Some(s) => {
                bits.iter_mut().zip(s).for_each(|(b, s)| *b |= s);
                false
            }
            None => true,
        });
        let key = if sees_all { &every } else { &bits };
        let row = match ids.get(key) {
            Some(&row) => row,
            None => {
                let next = ids.len() as u32;
                rows.extend_from_slice(key);
                ids.insert(key.clone(), next);
                next
            }
        };
        cells.push(row);
    }
    (rows, cells)
}

fn every_object(objects: usize) -> Vec<u64> {
    let mut every = vec![u64::MAX; objects / 64];
    if !objects.is_multiple_of(64) {
        every.push((1u64 << (objects % 64)) - 1);
    }
    every
}
