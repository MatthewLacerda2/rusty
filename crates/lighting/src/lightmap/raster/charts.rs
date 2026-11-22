//! A chart is an island of a mesh's lightmap UV. An invalid texel is filled only from valid texels of its own chart, so a floor's edge never takes the dark chart beside it.

use std::collections::BTreeMap;

use super::fill::NEIGHBOURS;
use super::{Surface, Texel, TexelPoint};

pub(in crate::lightmap) const NO_CHART: u32 = u32::MAX;

/// Triangles sharing a lightmap-UV corner (bit for bit) are one chart.
pub(in crate::lightmap) fn chart_of_triangles(mesh: &Surface) -> Vec<u32> {
    let tris = mesh.indices.len() / 3;
    let mut parent: Vec<usize> = (0..tris).collect();
    let mut corner: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for (k, t) in mesh.indices.chunks_exact(3).enumerate() {
        for &i in t {
            let uv = mesh.uvs[i as usize];
            let key = (uv.x.to_bits(), uv.y.to_bits());
            let other = *corner.entry(key).or_insert(k);
            union(&mut parent, k, other);
        }
    }
    let mut label = vec![NO_CHART; tris];
    let mut next = 0;
    for k in 0..tris {
        let root = find(&mut parent, k);
        if label[root] == NO_CHART {
            label[root] = next;
            next += 1;
        }
        label[k] = label[root];
    }
    label
}

fn find(parent: &mut [usize], mut k: usize) -> usize {
    while parent[k] != k {
        parent[k] = parent[parent[k]];
        k = parent[k];
    }
    k
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (a, b) = (find(parent, a), find(parent, b));
    // The lower root wins, so labels never depend on the order of unions.
    parent[a.max(b)] = a.min(b);
}

pub(in crate::lightmap) fn texel_charts(
    points: &[TexelPoint],
    charts: &[u32],
    size: u32,
) -> Vec<u32> {
    let mut labels = vec![NO_CHART; (size * size) as usize];
    for p in points {
        labels[p.at()] = charts[p.tri() / 3];
    }
    labels
}

/// A texel no ring reaches gets the chart's mean; a chart with no filled texel is left for dilation.
pub(in crate::lightmap) fn fill_within<T: Texel>(
    texels: &mut [T],
    filled: &mut [bool],
    labels: &[u32],
    size: u32,
) -> usize {
    let mut means: BTreeMap<u32, (T, f32)> = BTreeMap::new();
    for (i, &c) in labels.iter().enumerate() {
        if c != NO_CHART {
            let mean = means.entry(c).or_insert((T::default(), 0.0));
            if filled[i] {
                *mean = (mean.0 + texels[i], mean.1 + 1.0);
            }
        }
    }
    let pending: Vec<usize> = (0..filled.len())
        .filter(|&i| labels[i] != NO_CHART && !filled[i])
        .collect();
    fill_rings(texels, filled, labels, size, &pending);
    for i in pending {
        let (sum, n) = means[&labels[i]];
        if !filled[i] && n > 0.0 {
            texels[i] = sum / n;
            filled[i] = true;
        }
    }
    means.values().filter(|(_, n)| *n == 0.0).count()
}

/// Linear in `pending` however many rings a buried region takes; rings sum in `NEIGHBOURS` order so no visiting order shows in the bytes.
fn fill_rings<T: Texel>(
    texels: &mut [T],
    filled: &mut [bool],
    labels: &[u32],
    size: u32,
    pending: &[usize],
) {
    let same = |i: usize| move |&j: &usize| labels[j] == labels[i];
    let mut queued = vec![false; filled.len()];
    let mut ring: Vec<usize> = Vec::new();
    for &i in pending {
        if near(i, size).filter(same(i)).any(|j| filled[j]) {
            queued[i] = true;
            ring.push(i);
        }
    }
    while !ring.is_empty() {
        let values: Vec<T> = ring
            .iter()
            .map(|&i| {
                let (mut sum, mut n) = (T::default(), 0.0);
                for j in near(i, size).filter(same(i)).filter(|&j| filled[j]) {
                    sum = sum + texels[j];
                    n += 1.0;
                }
                sum / n
            })
            .collect();
        for (&i, value) in ring.iter().zip(values) {
            texels[i] = value;
            filled[i] = true;
        }
        let mut next = Vec::new();
        for &i in &ring {
            for j in near(i, size).filter(same(i)) {
                if !filled[j] && !queued[j] {
                    queued[j] = true;
                    next.push(j);
                }
            }
        }
        ring = next;
    }
}

fn near(i: usize, size: u32) -> impl Iterator<Item = usize> {
    let size = size as i32;
    let (x, y) = (i as i32 % size, i as i32 / size);
    NEIGHBOURS.into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        let inside = nx >= 0 && ny >= 0 && nx < size && ny < size;
        inside.then_some((ny * size + nx) as usize)
    })
}
