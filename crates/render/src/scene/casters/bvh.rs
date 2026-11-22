use std::collections::{BTreeSet, HashMap};

use glam::Vec3;

use crate::Frustum;

/// Entries a leaf holds at most.
const LEAF: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    id: u32,
    lo: Vec3,
    hi: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    first: u32,
    count: u32,
}

#[derive(Default)]
pub(crate) struct CasterIndex {
    slots: HashMap<u32, u32>,
    entries: Vec<Entry>,
    nodes: Vec<Node>,
    loose: Vec<u32>,
    per_frame: Vec<u32>,
    strays: BTreeSet<u32>,
}

impl CasterIndex {
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

    pub(crate) fn touch(&mut self, id: u32) {
        self.slots.remove(&id);
        self.strays.insert(id);
    }

    pub(crate) fn strays(&self) -> usize {
        self.strays.len()
    }

    pub(crate) fn near(&self, frustums: &[Frustum]) -> Vec<u32> {
        let mut ids = Vec::new();
        for frustum in frustums {
            self.visit(frustum, |e| ids.push(self.entries[e as usize].id));
        }
        ids
    }

    pub(crate) fn per_frame(&self) -> Vec<u32> {
        self.per_frame.clone()
    }

    pub(crate) fn unanswered(&self) -> impl Iterator<Item = u32> + '_ {
        self.loose.iter().chain(&self.strays).copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn entry(&self, id: u32, bounds: Option<(Vec3, Vec3)>) -> Option<u32> {
        let at = *self.slots.get(&id)?;
        let e = &self.entries[at as usize];
        (bounds == Some((e.lo, e.hi))).then_some(at)
    }

    #[cfg(test)]
    pub(crate) fn id(&self, entry: u32) -> u32 {
        self.entries[entry as usize].id
    }

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
