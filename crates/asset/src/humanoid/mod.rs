//! Retargeting clips between humanoid rigs, after Godot 4's `BoneMap` and Unity's Humanoid
//! avatar in a simpler form: a model's sidecar may carry [`HumanoidSettings`], a map from
//! [`CANONICAL`] bones to its own, and `clips_from` models whose clips play on this one.
//! Unmapped bones are guessed from common names (`guess`), else said once per import.
//!
//! The retarget runs at import (`borrow`): the result is an ordinary clip cached with the
//! target's, so blend trees, IK and root motion see a plain clip.

pub mod borrow;
mod guess;
mod retarget;

pub use borrow::borrow_clips;
pub use retarget::retarget;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::mesh_data::SkinData;
use rusty_core::fs::written::Stale;

pub const CANONICAL: &[&str] = &[
    "Hips",
    "Spine",
    "Chest",
    "UpperChest",
    "Neck",
    "Head",
    "LeftShoulder",
    "LeftUpperArm",
    "LeftLowerArm",
    "LeftHand",
    "RightShoulder",
    "RightUpperArm",
    "RightLowerArm",
    "RightHand",
    "LeftUpperLeg",
    "LeftLowerLeg",
    "LeftFoot",
    "LeftToes",
    "RightUpperLeg",
    "RightLowerLeg",
    "RightFoot",
    "RightToes",
];

const OPTIONAL: &[&str] = &[
    "UpperChest",
    "LeftShoulder",
    "RightShoulder",
    "LeftToes",
    "RightToes",
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanoidSettings {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bones: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips_from: Vec<String>,
}

pub fn clips_follow(file: &str, changed: &Stale) -> bool {
    changed.matches(file)
        || super::sidecar::load(std::path::Path::new(file)).is_ok_and(|s| {
            s.humanoid
                .clips_from
                .iter()
                .any(|from| changed.matches(from))
        })
}

impl HumanoidSettings {
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty() && self.clips_from.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneMap {
    pub slots: Vec<Option<usize>>,
    pub problems: Vec<String>,
}

impl BoneMap {
    pub fn resolve(skin: &SkinData, settings: &HumanoidSettings) -> Self {
        let mut problems = Vec::new();
        let mut explicit: Vec<Option<Option<usize>>> = vec![None; CANONICAL.len()];
        for (canonical, bone) in &settings.bones {
            let Some(i) = CANONICAL.iter().position(|c| c == canonical) else {
                problems.push(format!("'{canonical}' is no humanoid bone"));
                continue;
            };
            let slot = (!bone.is_empty())
                .then(|| skin.names.iter().position(|n| n == bone))
                .flatten();
            if !bone.is_empty() && slot.is_none() {
                problems.push(format!("{canonical}: no bone '{bone}'"));
            }
            if let Some(j) = slot.and_then(|s| explicit.iter().position(|e| *e == Some(Some(s)))) {
                problems.push(format!(
                    "{} and {canonical} both name '{bone}'",
                    CANONICAL[j]
                ));
            }
            explicit[i] = Some(slot);
        }
        let claimed = |slot: usize| explicit.contains(&Some(Some(slot)));
        let plain = guess::guess(&skin.names, |_| false);
        let guessed = guess::guess(&skin.names, claimed);
        for (i, (e, g)) in explicit.iter().zip(&plain).enumerate() {
            let Some(g) = g.filter(|g| e.is_none() && claimed(*g)) else {
                continue;
            };
            let now = guessed[i].map_or("left unmapped".into(), |s| {
                format!("guessed '{}' instead", skin.name(s))
            });
            problems.push(format!(
                "{}: '{}' is named for another bone; {now}",
                CANONICAL[i],
                skin.name(g)
            ));
        }
        let slots: Vec<Option<usize>> = (explicit.iter().zip(&guessed))
            .map(|(e, g)| e.unwrap_or(*g))
            .collect();
        let missing: Vec<&str> = (CANONICAL.iter().zip(&slots))
            .filter(|(c, s)| s.is_none() && !OPTIONAL.contains(c))
            .map(|(c, _)| *c)
            .collect();
        if !missing.is_empty() {
            problems.push(format!("unmapped: {}", missing.join(", ")));
        }
        Self { slots, problems }
    }

    pub fn shares_any(&self, other: &BoneMap) -> bool {
        (self.slots.iter().zip(&other.slots)).any(|(a, b)| a.is_some() && b.is_some())
    }

    pub fn slot(&self, name: &str) -> Option<usize> {
        let i = CANONICAL.iter().position(|c| *c == name)?;
        self.slots.get(i).copied().flatten()
    }

    pub fn canonical_of(&self, slot: usize) -> Option<usize> {
        self.slots.iter().position(|s| *s == Some(slot))
    }
}

#[cfg(test)]
mod guess_tests;
#[cfg(test)]
mod retarget_pose_tests;
#[cfg(test)]
mod retarget_tests;
/// Test hook, compiled in every build: the root's `app` tests build rigs with it.
#[doc(hidden)]
pub mod rig_fixture;
#[cfg(test)]
pub(crate) mod tests;
