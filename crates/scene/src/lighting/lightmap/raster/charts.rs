//! A lightmap's charts, and filling a chart's invalid texels from its own valid
//! ones (#1949).
//!
//! A chart is one island of a mesh's lightmap UV: the triangles joined through
//! shared UV corners. A texel the bake judges inside geometry (`trace::validity`)
//! is dropped, and [`fill_within`] fills it from valid texels of its own chart
//! only, as Unity dilates an invalid texel within its chart: a texel on a floor's
//! edge is never filled from the dark chart packed beside it in the map.

use std::collections::BTreeMap;

use super::{Surface, Texel, TexelPoint, NEIGHBOURS};

/// The label of a texel no triangle covers.
pub(in crate::lighting::lightmap) const NO_CHART: u32 = u32::MAX;

/// Per triangle of `mesh`, in index order, the chart it belongs to: triangles that
/// share a lightmap-UV corner (bit for bit) are one chart. Labels run from 0 in
/// order of each chart's first triangle.
pub(in crate::lighting::lightmap) fn chart_of_triangles(mesh: &Surface) -> Vec<u32> {
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

/// Per texel of a `size`² map, the chart of the triangle `points` took it from
/// ([`NO_CHART`] where none did); `charts` is [`chart_of_triangles`]' answer.
pub(in crate::lighting::lightmap) fn texel_charts(
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

/// Fill every texel a chart covers that is not `filled` (an invalid one) from the
/// filled texels of the same chart: ring by ring, each the average of its filled
/// eight neighbours in its chart, and one no ring reaches (cut off from the rest
/// of its chart) with the chart's mean. A chart with no filled texel at all is
/// left as it is, for the map's dilation to fill as before; the count of such
/// charts is returned.
pub(in crate::lighting::lightmap) fn fill_within<T: Texel>(
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

/// [`fill_within`]'s rings, as a frontier: a ring is the unfilled texels of a
/// chart beside the ring before it, so each texel is visited a bounded number of
/// times and the work is linear in `pending` (the invalid texels), however many
/// rings a buried region takes. Each ring reads only texels filled before it, and
/// sums its neighbours in `NEIGHBOURS` order, so neither the order within a ring
/// nor the queue shows in the bytes.
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

/// The indices of texel `i`'s eight neighbours inside a `size`² map, in
/// `NEIGHBOURS` order.
fn near(i: usize, size: u32) -> impl Iterator<Item = usize> {
    let size = size as i32;
    let (x, y) = (i as i32 % size, i as i32 / size);
    NEIGHBOURS.into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        let inside = nx >= 0 && ny >= 0 && nx < size && ny < size;
        inside.then_some((ny * size + nx) as usize)
    })
}
