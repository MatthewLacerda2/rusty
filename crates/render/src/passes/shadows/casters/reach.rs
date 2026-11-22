//! The index answers a candidate only while its bounds are those it was indexed with; the rest
//! go to every volume, so a volume keeps what testing every caster keeps.

use std::ops::Range;

use glam::Mat4;

use super::cull::Candidate;
use crate::scene::CasterIndex;
use crate::Frustum;

/// No candidate holds this index entry.
const NONE: u32 = u32::MAX;

pub(super) struct Reach<'a> {
    index: Option<&'a CasterIndex>,
    groups: Vec<Range<usize>>,
    by_entry: Vec<u32>,
    always: Vec<u32>,
}

impl<'a> Reach<'a> {
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
        let index = index.filter(|_| always.len() < groups.len());
        Self {
            index,
            groups,
            by_entry,
            always,
        }
    }

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
