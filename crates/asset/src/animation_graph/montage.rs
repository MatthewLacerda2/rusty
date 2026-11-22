//! Unreal's Animation Montages: a clip played once into a named, masked slot above every graph
//! layer; sections chain by `next` (itself loops, none ends the montage).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::event::check_events;
use super::{AnimationEvent, AnimationGraph, LayerBlending};

/// Full-body slot every animator has undeclared (Unreal's name); undeclared it sits above the
/// declared slots.
pub const DEFAULT_SLOT: &str = "DefaultSlot";

/// Default blend in/out time in seconds: Unreal's.
pub const DEFAULT_MONTAGE_BLEND: f32 = 0.25;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphSlot {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask: Vec<String>,
    #[serde(default)]
    pub blending: LayerBlending,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MontageSection {
    pub name: String,
    /// Clip seconds. A section plays up to the next section's start (the last
    /// one to the clip's end).
    pub time: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
}

impl Montage {
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
    pub fn montage(&self, name: &str) -> Option<&Montage> {
        self.montages.iter().find(|m| m.name == name)
    }

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

fn implicit_default_slot() -> GraphSlot {
    GraphSlot {
        name: DEFAULT_SLOT.to_string(),
        mask: Vec::new(),
        blending: LayerBlending::Override,
    }
}

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
