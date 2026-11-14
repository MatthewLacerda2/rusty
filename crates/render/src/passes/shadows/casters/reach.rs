//! Which of a sweep's candidates each light volume tests (#1874): the ones the
//! scene's caster index (`render::scene::CasterIndex`) reaches, and every one it
//! cannot answer for, in the order the gather gave them.
//!
//! The index holds the static objects as they were when the scene's statics last
//! changed; a candidate is answered by it only when its bounds are the ones it was
//! indexed with. Everything else (a dynamic or skinned caster, a moved static one,
//! one with no bounds) is tested by every volume, as before the index. Since the
//! index reaches every entry its volume could keep, a volume keeps the same casters
//! in the same order as testing them all: only `tested` and `frustum` shrink.

use std::ops::Range;

use glam::Mat4;

use super::cull::Candidate;
use crate::scene::CasterIndex;
use crate::Frustum;

/// No candidate holds this entry.
const NONE: u32 = u32::MAX;

/// A sweep's candidates grouped per entity, and how the index reaches them.
pub(super) struct Reach<'a> {
    index: Option<&'a CasterIndex>,
    /// Each entity's runs in the candidates, in gather order.
    groups: Vec<Range<usize>>,
    /// Per index entry, the group whose entity it indexes, or [`NONE`].
    by_entry: Vec<u32>,
    /// The groups the index cannot answer for, in order.
    always: Vec<u32>,
}

impl<'a> Reach<'a> {
    /// `candidates` against `index`; with none, every volume tests them all.
    pub(super) fn new(candidates: &[Candidate], index: Option<&'a CasterIndex>) -> Self {
        let mut groups: Vec<Range<usize>> = Vec::new();
        for (at, c) in candidates.iter().enumerate() {
            match groups.last_mut() {
                Some(g) if candidates[g.start].id == c.id => g.end = at + 1,
                _ => groups.push(at..at + 1),
            }
        }
        let mut by_entry = vec![NONE; index.map_or(0, CasterIndex::len)];
        let mut always = Vec::new();
        for (g, runs) in groups.iter().enumerate() {
            let c = &candidates[runs.start];
            match index.and_then(|i| i.entry(c.id, c.bounds)) {
                Some(e) => by_entry[e as usize] = g as u32,
                None => always.push(g as u32),
            }
        }
        // A sweep the index answers for no candidate of (a per-frame one, mostly)
        // visits no node.
        let index = index.filter(|_| always.len() < groups.len());
        Self {
            index,
            groups,
            by_entry,
            always,
        }
    }

    /// The candidates the volume of `view_proj` tests, as runs of the candidates,
    /// in gather order.
    pub(super) fn of(&self, view_proj: Mat4) -> Vec<Range<usize>> {
        let Some(index) = self.index else {
            return self.groups.clone();
        };
        let mut picked = self.always.clone();
        index.visit(&Frustum::from_view_proj(view_proj), |e| {
            let g = self.by_entry[e as usize];
            if g != NONE {
                picked.push(g);
            }
        });
        picked.sort_unstable();
        picked
            .iter()
            .map(|&g| self.groups[g as usize].clone())
            .collect()
    }
}
