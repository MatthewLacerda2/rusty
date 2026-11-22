use glam::Vec3;

const BINS: usize = 16;
const MAX_LEAF: usize = 8;
pub(crate) const MAX_DEPTH: usize = 48;
/// The entries a closest-first walk's stack needs over a tree at most [`MAX_DEPTH`]
/// deep: one pending far child per level, and the near one. `rt/trace.wgsl`'s
/// `RT_STACK` is the same number (`build_tests`).
#[cfg(test)]
pub(crate) const STACK: usize = MAX_DEPTH + 1;

/// One node of a flattened tree, as the shader reads it: a leaf holds `count`
/// primitives from `first`; an inner node (`count` 0) has children `first` and `first + 1`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Node {
    pub lo: [f32; 3],
    pub first: u32,
    pub hi: [f32; 3],
    pub count: u32,
}

#[derive(Clone, Copy)]
struct Prim {
    lo: Vec3,
    hi: Vec3,
    centre: Vec3,
    index: u32,
}

#[derive(Clone, Copy)]
struct Bounds {
    lo: Vec3,
    hi: Vec3,
}

impl Bounds {
    const EMPTY: Self = Self {
        lo: Vec3::splat(f32::INFINITY),
        hi: Vec3::splat(f32::NEG_INFINITY),
    };

    fn grow(self, lo: Vec3, hi: Vec3) -> Self {
        Self {
            lo: self.lo.min(lo),
            hi: self.hi.max(hi),
        }
    }

    fn area(self) -> f32 {
        let d = (self.hi - self.lo).max(Vec3::ZERO);
        d.x * d.y + d.y * d.z + d.z * d.x
    }
}

/// A binned-SAH tree over `bounds`, and the order its leaves name them in.
pub(crate) fn build(bounds: &[(Vec3, Vec3)]) -> (Vec<Node>, Vec<u32>) {
    let mut prims: Vec<Prim> = bounds
        .iter()
        .enumerate()
        .map(|(i, &(lo, hi))| Prim {
            lo,
            hi,
            centre: (lo + hi) * 0.5,
            index: i as u32,
        })
        .collect();
    if prims.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut nodes = vec![Node::default()];
    split(&mut nodes, &mut prims, (0, 0, 0));
    (nodes, prims.iter().map(|p| p.index).collect())
}

fn split(nodes: &mut Vec<Node>, prims: &mut [Prim], (at, base, depth): (usize, u32, usize)) {
    let b = prims.iter().fold(Bounds::EMPTY, |b, p| b.grow(p.lo, p.hi));
    nodes[at] = Node {
        lo: b.lo.to_array(),
        first: base,
        hi: b.hi.to_array(),
        count: prims.len() as u32,
    };
    let n = prims.len();
    if n <= 2 || depth >= MAX_DEPTH {
        return;
    }
    let mid = match best_split(prims, b) {
        Some((axis, at, cost)) if cost < n as f32 || n > MAX_LEAF => {
            partition(prims, axis, at).clamp(1, n - 1)
        }
        Some(_) => return,
        None if n > MAX_LEAF => n / 2,
        None => return,
    };
    let left = nodes.len();
    nodes.extend([Node::default(); 2]);
    nodes[at].first = left as u32;
    nodes[at].count = 0;
    let (l, r) = prims.split_at_mut(mid);
    split(nodes, l, (left, base, depth + 1));
    split(nodes, r, (left + 1, base + mid as u32, depth + 1));
}

/// The cheapest (axis, bin edge, cost) by the surface-area heuristic, in units of one
/// primitive test; `None` when every centre sits at one point.
fn best_split(prims: &[Prim], b: Bounds) -> Option<(usize, f32, f32)> {
    let c = prims
        .iter()
        .fold(Bounds::EMPTY, |c, p| c.grow(p.centre, p.centre));
    let mut best: Option<(usize, f32, f32)> = None;
    for axis in 0..3 {
        let (lo, hi) = (c.lo[axis], c.hi[axis]);
        if hi - lo <= f32::EPSILON * hi.abs().max(1.0) {
            continue;
        }
        let scale = BINS as f32 / (hi - lo);
        let bin = |p: &Prim| (((p.centre[axis] - lo) * scale) as usize).min(BINS - 1);
        let mut bins = [(Bounds::EMPTY, 0u32); BINS];
        for p in prims {
            let slot = &mut bins[bin(p)];
            *slot = (slot.0.grow(p.lo, p.hi), slot.1 + 1);
        }
        let mut right = [(0.0, 0u32); BINS];
        let mut acc = (Bounds::EMPTY, 0);
        for i in (1..BINS).rev() {
            acc = (acc.0.grow(bins[i].0.lo, bins[i].0.hi), acc.1 + bins[i].1);
            right[i] = (acc.0.area(), acc.1);
        }
        let mut left = (Bounds::EMPTY, 0);
        for i in 0..BINS - 1 {
            left = (left.0.grow(bins[i].0.lo, bins[i].0.hi), left.1 + bins[i].1);
            let (r_area, r_n) = right[i + 1];
            if left.1 == 0 || r_n == 0 {
                continue;
            }
            let cost = 1.0 + (left.0.area() * left.1 as f32 + r_area * r_n as f32) / b.area();
            if best.is_none_or(|(_, _, c)| cost < c) {
                best = Some((axis, lo + (i + 1) as f32 / scale, cost));
            }
        }
    }
    best.filter(|(_, _, cost)| cost.is_finite())
}

fn partition(prims: &mut [Prim], axis: usize, at: f32) -> usize {
    let mut i = 0;
    for j in 0..prims.len() {
        if prims[j].centre[axis] < at {
            prims.swap(i, j);
            i += 1;
        }
    }
    i
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
