//! src/components/animator/montage/resolve.rs — turning a name into a
//! montage ready to start (#1894): a montage of the graph, else a clip
//! played as a one-section montage in `DefaultSlot`.

use super::{find, ActiveMontage, DEFAULT_SECTION};
use crate::asset::anim_data::AnimationClip;
use crate::asset::animation_graph::{AnimationGraph, Montage, MontageSection, DEFAULT_SLOT};

/// Why a montage would not start.
#[derive(Clone, Debug, PartialEq)]
pub enum MontageRefused {
    /// No montage of that name, and no clip either.
    Unknown,
    /// The montage's clip is not on the mesh.
    NoClip(String),
    NoSlot(String),
    NoSection(String),
}

impl ActiveMontage {
    /// Resolve `name` against `graph`'s montages, else as a clip name in
    /// `clips` (played as a one-section montage), into a montage ready to
    /// start in `slot` (the montage's own when `None`) from `start` (its first
    /// section when `None`) at `rate`.
    pub fn resolve(
        graph: Option<&AnimationGraph>,
        clips: &[AnimationClip],
        name: &str,
        (slot, start, rate): (Option<&str>, Option<&str>, f32),
    ) -> Result<Self, MontageRefused> {
        let clip = |n: &str| clips.iter().find(|c| c.name == n);
        let mut montage = match graph.and_then(|g| g.montage(name)) {
            Some(m) => m.clone(),
            None if clip(name).is_some() => Montage::of_clip(name, DEFAULT_SLOT),
            None => return Err(MontageRefused::Unknown),
        };
        if let Some(slot) = slot {
            montage.slot = slot.to_string();
        }
        let length = clip(&montage.clip)
            .ok_or_else(|| MontageRefused::NoClip(montage.clip.clone()))?
            .duration;
        let default = AnimationGraph::default();
        let (order, slot) = graph
            .unwrap_or(&default)
            .slot(&montage.slot)
            .ok_or_else(|| MontageRefused::NoSlot(montage.slot.clone()))?;
        if montage.sections.is_empty() {
            montage.sections.push(MontageSection {
                name: DEFAULT_SECTION.to_string(),
                time: 0.0,
                next: None,
            });
        }
        let section = match start {
            None => 0,
            Some(s) => find(&montage.sections, s)
                .ok_or_else(|| MontageRefused::NoSection(s.to_string()))?,
        };
        let sections = &montage.sections;
        let next = sections
            .iter()
            .map(|s| s.next.as_deref().and_then(|n| find(sections, n)))
            .collect();
        let mut active = Self {
            time: 0.0,
            montage,
            slot,
            order,
            length,
            rate,
            section,
            next,
            elapsed: 0.0,
            out: None,
        };
        active.time = active.start(section);
        Ok(active)
    }

    /// Blend in and out over these times instead of its own (`PlayMontage`'s
    /// `blend_in` / `blend_out`); a negative or non-finite one is ignored.
    pub fn with_blends(mut self, blend_in: Option<f32>, blend_out: Option<f32>) -> Self {
        let valid = |b: Option<f32>| b.filter(|b| b.is_finite() && *b >= 0.0);
        if let Some(b) = valid(blend_in) {
            self.montage.blend_in = b;
        }
        if let Some(b) = valid(blend_out) {
            self.montage.blend_out = b;
        }
        self
    }
}
