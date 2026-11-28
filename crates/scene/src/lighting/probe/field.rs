//! A point in a set's grid reads the first such grid; any other blends the nearest probes of
//! every set.

use glam::{Vec3, Vec4};

use super::index::FOUND;
use super::sample::{blend, Blend, Found, NONE_FOUND};
use super::{Probe, ProbeVolume};
use crate::lighting::sh::Sh9;
use crate::revision::Revision;

#[derive(Clone, Copy)]
pub struct ProbeField<'a> {
    sets: &'a [(u32, &'a ProbeVolume)],
}

impl<'a> ProbeField<'a> {
    /// `(scene handle, set)`, in load order.
    pub fn new(sets: &'a [(u32, &'a ProbeVolume)]) -> Self {
        Self { sets }
    }

    pub fn sets(&self) -> impl Iterator<Item = (u32, &'a ProbeVolume)> + '_ {
        self.sets.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.sets.iter().all(|(_, v)| v.is_empty())
    }

    pub fn probes(&self) -> impl Iterator<Item = &'a Probe> + '_ {
        self.sets.iter().flat_map(|(_, v)| v.probes())
    }

    pub fn revisions(&self) -> Vec<(u32, Revision)> {
        self.sets.iter().map(|(h, v)| (*h, v.revision())).collect()
    }

    pub fn sample(&self, position: Vec3) -> Option<Sh9> {
        self.sample_by(position, |p| p.sh)
    }

    pub fn sample_occlusion(&self, position: Vec3) -> Option<Vec4> {
        self.sample_by(position, |p| p.occlusion)
    }

    fn sample_by<T: Blend>(&self, position: Vec3, get: impl Fn(&Probe) -> T) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let filled = || self.sets.iter().filter(|(_, v)| !v.is_empty());
        if let Some(v) = filled().find_map(|(_, v)| v.in_grid(position, &get)) {
            return Some(v);
        }
        Some(blend(&self.found(position, |_| true), &get))
    }

    /// A tie goes to the earlier set.
    fn found(&self, position: Vec3, keep: impl Fn(u32) -> bool) -> Found<'a> {
        let mut near = NONE_FOUND;
        for (_, set) in self.sets.iter().filter(|(h, _)| keep(*h)) {
            for (d, p) in set.found(position) {
                let Some(p) = p else { break };
                let mut at = FOUND;
                while at > 0 && d < near[at - 1].0 {
                    at -= 1;
                }
                if at < FOUND {
                    near.copy_within(at..FOUND - 1, at + 1);
                    near[at] = (d, Some(p));
                }
            }
        }
        near
    }

    /// Metres to the farthest probe `position` blends among the sets `keep` takes.
    pub fn reach_without(&self, position: Vec3, keep: impl Fn(u32) -> bool) -> f32 {
        self.found(position, keep)[FOUND - 1].0.sqrt()
    }
}

impl ProbeVolume {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let finite = self
            .probes
            .iter()
            .map(|p| p.position)
            .filter(|p| p.is_finite());
        finite.fold(None, |b, p| match b {
            None => Some((p, p)),
            Some((lo, hi)) => Some((lo.min(p), hi.max(p))),
        })
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
