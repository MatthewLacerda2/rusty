//! crates/asset/src/animation_graph/montage.rs — slots and montages (#1894).
//!
//! Unreal's Animation Montages, declared on the graph. A **slot** is a named,
//! masked layer that sits above every graph layer and shows only while a montage
//! plays in it; every animator also has the implicit full-body
//! [`DEFAULT_SLOT`]. A **montage** is a clip played once into a slot over the
//! running state machine, with blend in/out times, optional **sections** (named
//! times in the clip, each going on to its `next` when it ends: itself loops it,
//! none ends the montage) and its own animation events. Playing one is the
//! runtime's job (`app::animation::montage`, `Animator.PlayMontage`); this is
//! the data and its load checks, each problem naming its path
//! (`montages[1].sections[2].next: no section 'Hodl'`).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::event::check_events;
use super::{AnimationEvent, AnimationGraph, LayerBlending};

/// The full-body slot every animator has without declaring it (Unreal's name).
/// Undeclared, it sits above the declared slots; declared, it takes its place
/// and mask like any other slot.
pub const DEFAULT_SLOT: &str = "DefaultSlot";

/// The blend in and out time of a montage that does not say (and of a clip
/// played as a montage), in seconds: Unreal's default.
pub const DEFAULT_MONTAGE_BLEND: f32 = 0.25;

/// One slot a montage plays into: a name, a mask and a blend mode, with a
/// layer's rules (`GraphLayer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphSlot {
    pub name: String,
    /// Bone names, each standing for its whole subtree; empty is the whole body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask: Vec<String>,
    #[serde(default)]
    pub blending: LayerBlending,
}

/// One section of a montage: a name, where it starts in the clip, and the
/// section it goes on to when it ends (itself loops it; `None` ends the montage).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MontageSection {
    pub name: String,
    /// Clip seconds. A section plays up to the next section's start (the last
    /// one to the clip's end).
    pub time: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// One montage: a clip played once into a slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Montage {
    pub name: String,
    pub clip: String,
    #[serde(default = "default_slot")]
    pub slot: String,
    #[serde(default = "default_blend")]
    pub blend_in: f32,
    #[serde(default = "default_blend")]
    pub blend_out: f32,
    /// In ascending time order. Empty is one section spanning the clip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<MontageSection>,
    /// Clip-timed markers, fired like a node's (`OnAnimationEvent`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
}

impl Montage {
    /// A clip played as a one-section montage in `slot`, at the default blends.
    pub fn of_clip(clip: &str, slot: &str) -> Self {
        Self {
            name: clip.to_string(),
            clip: clip.to_string(),
            slot: slot.to_string(),
            blend_in: DEFAULT_MONTAGE_BLEND,
            blend_out: DEFAULT_MONTAGE_BLEND,
            sections: Vec::new(),
            events: Vec::new(),
        }
    }
}

fn default_slot() -> String {
    DEFAULT_SLOT.to_string()
}

fn default_blend() -> f32 {
    DEFAULT_MONTAGE_BLEND
}

impl AnimationGraph {
    /// The montage named `name`.
    pub fn montage(&self, name: &str) -> Option<&Montage> {
        self.montages.iter().find(|m| m.name == name)
    }

    /// The slot named `name` and its place in the slot stack: a declared slot's
    /// index, or the implicit full-body [`DEFAULT_SLOT`] above them all.
    pub fn slot(&self, name: &str) -> Option<(usize, GraphSlot)> {
        if let Some(i) = self.slots.iter().position(|s| s.name == name) {
            return Some((i, self.slots[i].clone()));
        }
        (name == DEFAULT_SLOT).then(|| (self.slots.len(), implicit_default_slot()))
    }

    pub(super) fn check_slots(&self, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        for (i, slot) in self.slots.iter().enumerate() {
            let label = format!("slots[{i}]");
            if slot.name.is_empty() {
                problems.push(format!("{label}.name: empty"));
            } else if !seen.insert(slot.name.as_str()) {
                problems.push(format!("{label}.name: duplicate slot '{}'", slot.name));
            }
            if slot.mask.iter().any(String::is_empty) {
                problems.push(format!("{label}.mask: an empty bone name"));
            }
        }
    }

    pub(super) fn check_montages(&self, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        for (i, montage) in self.montages.iter().enumerate() {
            let label = format!("montages[{i}]");
            if montage.name.is_empty() {
                problems.push(format!("{label}.name: empty"));
            } else if !seen.insert(montage.name.as_str()) {
                problems.push(format!(
                    "{label}.name: duplicate montage '{}'",
                    montage.name
                ));
            }
            if montage.clip.is_empty() {
                problems.push(format!("{label}.clip: empty"));
            }
            if self.slot(&montage.slot).is_none() {
                problems.push(format!("{label}.slot: no slot '{}'", montage.slot));
            }
            for (field, value) in [
                ("blend_in", montage.blend_in),
                ("blend_out", montage.blend_out),
            ] {
                if !value.is_finite() || value < 0.0 {
                    problems.push(format!("{label}.{field}: negative or non-finite"));
                }
            }
            check_sections(&label, &montage.sections, problems);
            check_events(&format!("{label}.events:"), &montage.events, problems);
        }
    }
}

/// The implicit [`DEFAULT_SLOT`]: whole body, override.
fn implicit_default_slot() -> GraphSlot {
    GraphSlot {
        name: DEFAULT_SLOT.to_string(),
        mask: Vec::new(),
        blending: LayerBlending::Override,
    }
}

/// Unique, named sections at finite, strictly ascending times, whose `next`
/// names one of them.
fn check_sections(label: &str, sections: &[MontageSection], problems: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut last = f32::NEG_INFINITY;
    for (i, section) in sections.iter().enumerate() {
        let at = format!("{label}.sections[{i}]");
        if section.name.is_empty() {
            problems.push(format!("{at}.name: empty"));
        } else if !seen.insert(section.name.as_str()) {
            problems.push(format!("{at}.name: duplicate section '{}'", section.name));
        }
        if !section.time.is_finite() || section.time < 0.0 {
            problems.push(format!("{at}.time: negative or non-finite"));
        } else if section.time <= last {
            problems.push(format!("{at}.time: not after the section before it"));
        } else {
            last = section.time;
        }
    }
    for (i, section) in sections.iter().enumerate() {
        if let Some(next) = &section.next {
            if !sections.iter().any(|s| &s.name == next) {
                problems.push(format!("{label}.sections[{i}].next: no section '{next}'"));
            }
        }
    }
}
