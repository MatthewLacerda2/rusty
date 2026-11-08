//! The static casters' bounding-volume hierarchy (#1874): which of a scene's
//! static objects a shadow view could reach, so a light's tile tests the few
//! casters near it and not every caster in the level.
//!
//! **Exact, not approximate.** A node's box is the union of its children's, and the
//! light's frustum test (`Frustum::intersects_aabb`, its positive vertex per plane)
//! is monotone in the box, in floating point too: a box that passes, every box
//! holding it passes. So [`CasterIndex::visit`] reaches every entry whose own box
//! passes, and exactly those: a caster the linear pass would keep is never missed. A
//! static object it cannot answer for (no finite bounds when built: `loose`; or
//! re-derived or moved since: `strays`) is handed to every volume instead.

use std::collections::{BTreeSet, HashMap};

use glam::Vec3;

use crate::render::Frustum;

/// Entries a leaf holds at most.
const LEAF: usize = 4;

/// One indexed object: its entity and the world bounds it was indexed with.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    id: u32,
    lo: Vec3,
    hi: Vec3,
}

/// One node: its box, and either `count` entries from `first` (a leaf) or, with
/// `count` zero, its children at the next node and at `first`.
#[derive(Clone, Copy, Debug)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    first: u32,
    count: u32,
}

/// The static objects' BVH.
#[derive(Default)]
pub(crate) struct CasterIndex {
    /// Entities by their entry; a stray's is dropped.
    slots: HashMap<u32, u32>,
    entries: Vec<Entry>,
    nodes: Vec<Node>,
    /// The objects built with no finite bounds.
    loose: Vec<u32>,
    /// The objects that cast from the per-frame sweeps when built.
    per_frame: Vec<u32>,
    /// The objects re-derived or moved since the build (`touch`).
    strays: BTreeSet<u32>,
}

impl CasterIndex {
    /// The index of `objects` (entities, their world bounds, and whether each
    /// casts from the per-frame sweeps); one with no finite bounds is kept loose.
    pub(crate) fn build(objects: impl Iterator<Item = (u32, Option<(Vec3, Vec3)>, bool)>) -> Self {
        let (mut entries, mut loose, mut per_frame) = (Vec::new(), Vec::new(), Vec::new());
        for (id, bounds, frame) in objects {
            if frame {
                per_frame.push(id);
            }
            match bounds.filter(|(lo, hi)| lo.is_finite() && hi.is_finite()) {
                Some((lo, hi)) => entries.push(Entry { id, lo, hi }),
                None => loose.push(id),
            }
        }
        let mut nodes = Vec::with_capacity(entries.len() / 2 + 1);
        if !entries.is_empty() {
            split(&mut nodes, &mut entries, 0);
        }
        let slots = entries.iter().enumerate();
        let slots = slots.map(|(i, e)| (e.id, i as u32)).collect();
        Self {
            slots,
            entries,
            nodes,
            loose,
            per_frame,
            strays: BTreeSet::new(),
        }
    }

    /// Object `id` was re-derived or moved: no entry answers for it any more.
    pub(crate) fn touch(&mut self, id: u32) {
        self.slots.remove(&id);
        self.strays.insert(id);
    }

    /// How many objects were touched since the build.
    pub(crate) fn strays(&self) -> usize {
        self.strays.len()
    }

    /// The entries any of `frustums` reach ([`Self::visit`]); unordered, maybe
    /// repeated.
    pub(crate) fn near(&self, frustums: &[Frustum]) -> Vec<u32> {
        let mut ids = Vec::new();
        for frustum in frustums {
            self.visit(frustum, |e| ids.push(self.entries[e as usize].id));
        }
        ids
    }

    /// The objects that cast from the per-frame sweeps when built.
    pub(crate) fn per_frame(&self) -> Vec<u32> {
        self.per_frame.clone()
    }

    /// The objects it cannot answer for: the loose ones and the strays.
    pub(crate) fn unanswered(&self) -> impl Iterator<Item = u32> + '_ {
        self.loose.iter().chain(&self.strays).copied()
    }

    /// How many entries it holds.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Entity `id`'s entry, when it was indexed with exactly `bounds`.
    pub(crate) fn entry(&self, id: u32, bounds: Option<(Vec3, Vec3)>) -> Option<u32> {
        let at = *self.slots.get(&id)?;
        let e = &self.entries[at as usize];
        (bounds == Some((e.lo, e.hi))).then_some(at)
    }

    /// The entity entry `entry` indexes.
    #[cfg(test)]
    pub(crate) fn id(&self, entry: u32) -> u32 {
        self.entries[entry as usize].id
    }

    /// Every entry whose own box `frustum` keeps: the nodes it culls hold none.
    pub(crate) fn visit(&self, frustum: &Frustum, mut hit: impl FnMut(u32)) {
        let mut stack = Vec::new();
        if !self.nodes.is_empty() {
            stack.push(0u32);
        }
        while let Some(at) = stack.pop() {
            let node = self.nodes[at as usize];
            if !frustum.intersects_aabb(node.lo, node.hi) {
                continue;
            }
            match node.count {
                0 => stack.extend([node.first, at + 1]),
                n => {
                    let leaf = node.first..node.first + n;
                    let entries = leaf.filter(|&e| {
                        let e = &self.entries[e as usize];
                        frustum.intersects_aabb(e.lo, e.hi)
                    });
                    entries.for_each(&mut hit);
                }
            }
        }
    }
}

/// Build the subtree over `entries` (which start at entry `base`) into `nodes`:
/// split at the median centre along the longest axis of the centres' extent.
fn split(nodes: &mut Vec<Node>, entries: &mut [Entry], base: u32) {
    let (lo, hi) = entries.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(lo, hi), e| (lo.min(e.lo), hi.max(e.hi)),
    );
    let at = nodes.len();
    let count = entries.len() as u32;
    nodes.push(Node {
        lo,
        hi,
        first: base,
        count,
    });
    if entries.len() <= LEAF {
        return;
    }
    let centre = |e: &Entry| e.lo + e.hi;
    let (clo, chi) = entries.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(lo, hi), e| (lo.min(centre(e)), hi.max(centre(e))),
    );
    let axis = (chi - clo).max_position();
    let mid = entries.len() / 2;
    entries.select_nth_unstable_by(mid, |a, b| centre(a)[axis].total_cmp(&centre(b)[axis]));
    let (left, right) = entries.split_at_mut(mid);
    split(nodes, left, base);
    let second = nodes.len() as u32;
    split(nodes, right, base + mid as u32);
    nodes[at].first = second;
    nodes[at].count = 0;
}

#[cfg(test)]
#[path = "bvh_tests.rs"]
mod tests;
