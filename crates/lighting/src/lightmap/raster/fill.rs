use super::Texel;

/// So bilinear filtering at a chart's edge reads lit texels, not black.
pub(in crate::lightmap) fn dilate<T: Texel>(
    texels: &mut [T],
    filled: &mut [bool],
    size: u32,
    passes: u32,
) {
    let size = size as i32;
    for _ in 0..passes {
        let snapshot = filled.to_vec();
        let values = texels.to_vec();
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) as usize;
                if snapshot[i] {
                    continue;
                }
                let (mut sum, mut n) = (T::default(), 0.0);
                for (dx, dy) in NEIGHBOURS {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= size || ny >= size {
                        continue;
                    }
                    let j = (ny * size + nx) as usize;
                    if snapshot[j] {
                        sum = sum + values[j];
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    texels[i] = sum / n;
                    filled[i] = true;
                }
            }
        }
    }
}

/// Nearest in four-neighbour steps, ties to the first in row-major order, so a mip averaging a
/// chart's border with the empty background reads the chart's light, not black. A map with no
/// filled texel stays as it is.
pub(in crate::lightmap) fn fill_rest<T: Texel>(texels: &mut [T], filled: &[bool], size: u32) {
    let size = size as usize;
    let mut done = filled.to_vec();
    let mut queue: std::collections::VecDeque<usize> =
        (0..done.len()).filter(|&i| done[i]).collect();
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % size, i / size);
        let near = [
            (x > 0).then(|| i - 1),
            (x + 1 < size).then(|| i + 1),
            (y > 0).then(|| i - size),
            (y + 1 < size).then(|| i + size),
        ];
        for j in near.into_iter().flatten() {
            if !done[j] {
                texels[j] = texels[i];
                done[j] = true;
                queue.push_back(j);
            }
        }
    }
}

pub(super) const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
