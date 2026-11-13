//! crates/asset/src/humanoid/ — retargeting clips between humanoid rigs (#1747).
//!
//! Godot 4's `BoneMap` + `SkeletonProfileHumanoid`, and Unity's Humanoid avatar
//! in a simpler form: a model's `.meta` sidecar may carry a [`HumanoidSettings`],
//! a map from the canonical humanoid bones ([`CANONICAL`]) to the model's own
//! bones, and a list of other models (`clips_from`) whose clips play on this one.
//! A bone the map doesn't name is guessed from common names (`guess`:
//! `mixamorig:`, `.L/.R`, `_l/_r`); what neither finds is left unmapped and said
//! once per import in the console.
//!
//! The retarget runs **at import** (`borrow`): each borrowed clip is rewritten
//! into the target's joint slots once (`retarget`), and the result is an ordinary
//! clip of the target's import, cached with it like its own clips. So blend
//! trees, layers, masks, IK and root motion see a plain clip, nothing samples per
//! bone per tick, and a clip on its own rig is never touched.

pub mod borrow;
mod guess;
mod retarget;

pub use borrow::borrow_clips;
pub use retarget::retarget;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::mesh_data::SkinData;
use rusty_core::fs::written::Stale;

/// The canonical humanoid bones, Unity's `HumanBodyBones` names, parents first.
/// The issue's set plus the optional shoulders and upper chest, so a rig that has
/// them keeps their motion rather than holding them at rest.
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

/// The canonical bones a rig without them still retargets: optional ones a
/// missing-bone warning leaves out.
const OPTIONAL: &[&str] = &[
    "UpperChest",
    "LeftShoulder",
    "RightShoulder",
    "LeftToes",
    "RightToes",
];

/// A model's humanoid import settings, in its sidecar under `humanoid`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanoidSettings {
    /// Canonical bone ([`CANONICAL`]) → the model's bone name. An entry overrides
    /// the guess for its canonical bone; an empty name unmaps it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bones: BTreeMap<String, String>,
    /// Model files whose clips are retargeted onto this model at import, in
    /// order: a clip whose name this model (or an earlier source) already has
    /// is skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips_from: Vec<String>,
}

/// Whether the clips of the model at `file` change with the files `changed`
/// names: it is one of them, or it borrows clips from one (each by any
/// spelling of the same file). The one test `Assets.SetHumanoid` and the
/// re-import of a re-written model share.
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

/// A skin's resolved humanoid map: the joint slot of each canonical bone found,
/// and what was not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneMap {
    /// Canonical index (into [`CANONICAL`]) → joint slot.
    pub slots: Vec<Option<usize>>,
    /// What resolving said: an explicit name not in the skeleton, an unknown
    /// canonical key, the required bones nothing found.
    pub problems: Vec<String>,
}

impl BoneMap {
    /// `skin`'s map: `settings.bones` first, the guesser for the rest over the
    /// joints the explicit entries leave free, so naming a joint for one bone
    /// takes it from the bone a guess gave it to (said in `problems`).
    pub fn resolve(skin: &SkinData, settings: &HumanoidSettings) -> Self {
        let mut problems = Vec::new();
        // Canonical index → its explicit entry's slot (`Some(None)`: unmapped).
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

    /// Whether some canonical bone is mapped on both `self` and `other`: a clip
    /// can move between their rigs at all.
    pub fn shares_any(&self, other: &BoneMap) -> bool {
        (self.slots.iter().zip(&other.slots)).any(|(a, b)| a.is_some() && b.is_some())
    }

    /// The slot mapped to canonical bone `name`.
    pub fn slot(&self, name: &str) -> Option<usize> {
        let i = CANONICAL.iter().position(|c| *c == name)?;
        self.slots.get(i).copied().flatten()
    }

    /// The canonical bone (index) joint `slot` is mapped to.
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
#[cfg(test)]
pub(crate) mod rig_fixture;
#[cfg(test)]
pub(crate) mod tests;
