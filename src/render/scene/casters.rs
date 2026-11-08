//! A scene's static shadow casters: their digest and the static bakes' key
//! (`RenderScene::statics_key`, part 2 of #1706), and their bounding-volume
//! hierarchy (#1874): which of them a shadow view could reach, so a light's tile
//! tests the few casters near it and not every caster in the level.
//!
//! Unity culls each shadow view over the scene's spatial structure, Unreal through
//! `FScene`'s primitive octree; this is a BVH over the static objects' world bounds,
//! rebuilt when the scene's [`StaticsKey`] moves (`RenderScene::refresh_casters`).
//! A static object that moved since, a dynamic or skinned one, or one whose bounds
//! are unknown or not finite is not in it: the sweep tests those linearly
//! (`passes::shadows::casters::reach`), so the index only ever narrows.
//!
//! **Exact, not approximate.** A node's box is the union of its children's, and the
//! light's frustum test (`Frustum::intersects_aabb`, its positive vertex per plane)
//! is monotone in the box, in floating point too: a box that passes, every box
//! holding it passes. So [`CasterIndex::visit`] reaches every entry whose own box
//! passes, and a caster the linear pass would test and keep is never missed.

use std::collections::HashMap;

use glam::Vec3;

use super::RenderScene;
use crate::render::passes::shadows::{casting_key, StaticsKey};
use crate::render::Frustum;
use crate::scene::Scene;

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
    /// Entities by their entry.
    slots: HashMap<u32, u32>,
    entries: Vec<Entry>,
    nodes: Vec<Node>,
}

impl CasterIndex {
    /// The index of `objects` (entities and their world bounds), leaving out any
    /// whose bounds are not finite.
    pub(crate) fn build(objects: impl Iterator<Item = (u32, (Vec3, Vec3))>) -> Self {
        let mut entries: Vec<Entry> = objects
            .map(|(id, (lo, hi))| Entry { id, lo, hi })
            .filter(|e| e.lo.is_finite() && e.hi.is_finite())
            .collect();
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
        }
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

    /// Every entry in a leaf whose box and every ancestor's `frustum` keeps; a
    /// superset of the entries whose own boxes it keeps.
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
                n => (node.first..node.first + n).for_each(&mut hit),
            }
        }
    }
}

impl RenderScene {
    /// The digest, refolded if forgotten.
    pub(super) fn refold_casting(&mut self) {
        if self.casting.is_none() {
            let statics = self.objects().filter(|(_, o)| o.is_static);
            let statics = statics.map(|(id, o)| (id, &o.casting));
            self.casting = Some(casting_key(statics, &self.unbaked));
        }
    }

    /// The static bakes' key for `scene` (whose objects these are) now: what a
    /// cascade layer or an atlas tile keeps beside what it baked.
    pub(crate) fn statics_key(&self, scene: &Scene) -> StaticsKey {
        let casting = self.casting.expect("an extract folds the digest");
        (scene.id(), scene.world.static_revision(), casting)
    }

    /// The caster index rebuilt over the static objects' bounds if `scene`'s
    /// statics key moved since it was built (#1874). A static object moved without
    /// refolding the key is answered linearly by the sweeps until then
    /// (`passes::shadows::casters::reach`).
    pub(super) fn refresh_casters(&mut self, scene: &Scene) {
        let key = self.statics_key(scene);
        if self.casters_key != Some(key) {
            let statics = self.objects().filter(|(_, o)| o.is_static);
            let bounded = statics.filter_map(|(id, o)| o.bounds.map(|b| (id, b)));
            self.casters = CasterIndex::build(bounded);
            self.casters_key = Some(key);
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
#[path = "casters_tests.rs"]
mod tests;
