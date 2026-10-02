//! src/components/animator/ik.rs — the Animator's IK constraints (#461).
//!
//! Inverse kinematics configured as settings on the Animator rather than a new
//! first-class component: a named list of constraints over the entity's bone
//! GameObjects (#453), each either a **two-bone** limb (Unity's `TwoBoneIK`: an
//! arm or a leg reaching a target, the elbow or knee bending toward a hint) or an
//! **aim chain** (Unity's `MultiAim` / `Animator.SetLookAt*`: spine → head turned
//! toward a point, spread over the chain by per-bone weights, clamped).
//!
//! The chain layout, weights and clamp are authored and saved; the target and the
//! hint are runtime state a script sets (`Animator.SetIKTarget*`), as Unity's
//! `SetIKPosition` is — a point, or an entity followed every step. The solver is
//! `app::ik`.

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// One named IK constraint on an animator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IkConstraint {
    /// How scripts address the constraint (`"LeftHand"`, `"Look"`); unique per
    /// animator.
    pub name: String,
    pub chain: IkChain,
    /// The constraint's global weight in `[0, 1]`. A two-bone limb's target is
    /// pulled this far from where the animation put the tip (Unity's
    /// `TwoBoneIK` target weight); an aim chain turns this fraction of the way.
    /// 0 is an exact no-op.
    #[serde(default = "full")]
    pub weight: f32,
    /// What the chain reaches for or aims at. Runtime only: `None` leaves the
    /// animated pose alone.
    #[serde(skip)]
    pub target: Option<IkTarget>,
    /// A two-bone limb's pole: the elbow or knee bends toward it. Runtime only;
    /// ignored by an aim chain. `None` keeps the animated bend plane.
    #[serde(skip)]
    pub hint: Option<IkTarget>,
    /// What the last solve wrote: `(bone, local rotation written, local rotation
    /// before)`. The next solve first puts back every bone nothing has rewritten
    /// since, so IK post-processes this tick's pose instead of compounding on its
    /// own last result (a bone no clip keys would otherwise ratchet past the
    /// clamp). Runtime only.
    #[serde(skip)]
    pub applied: Vec<(u32, Quat, Quat)>,
}

/// The bones a constraint moves, by bone name, and how.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IkChain {
    /// An arm or leg: `root` (upper arm / thigh) and `mid` (forearm / shin)
    /// rotate so `tip` (hand / foot) lands on the target. The tip keeps its
    /// local rotation.
    TwoBone {
        root: String,
        mid: String,
        tip: String,
    },
    /// A chain listed root → tip (spine → chest → neck → head) whose last bone's
    /// `axis` turns toward the target.
    Aim {
        bones: Vec<String>,
        /// Each bone's share of the turn, parallel to `bones`. Empty spreads it
        /// evenly.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        weights: Vec<f32>,
        /// The aiming direction in the last bone's local space (glTF forward,
        /// +Z, by default).
        #[serde(default = "forward")]
        axis: Vec3,
        /// The most the chain turns away from the animated aim, in degrees.
        #[serde(default = "half_turn")]
        clamp_degrees: f32,
    },
}

/// Where a constraint reaches: a world-space point, or an entity's live world
/// position (a weapon's foregrip, the player's head).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IkTarget {
    Point(Vec3),
    Entity(u32),
}

fn full() -> f32 {
    1.0
}

fn forward() -> Vec3 {
    Vec3::Z
}

fn half_turn() -> f32 {
    180.0
}

impl IkConstraint {
    /// A new constraint at full weight, with no target yet.
    pub fn new(name: impl Into<String>, chain: IkChain) -> Self {
        Self {
            name: name.into(),
            chain,
            weight: 1.0,
            target: None,
            hint: None,
            applied: Vec::new(),
        }
    }

    /// An aim chain with the defaults: even weights, +Z aim, no clamp.
    pub fn aim(name: impl Into<String>, bones: Vec<String>) -> Self {
        let chain = IkChain::Aim {
            bones,
            weights: Vec::new(),
            axis: forward(),
            clamp_degrees: half_turn(),
        };
        Self::new(name, chain)
    }

    /// The bone names the constraint moves or reads, root first.
    pub fn bone_names(&self) -> Vec<&str> {
        match &self.chain {
            IkChain::TwoBone { root, mid, tip } => vec![root, mid, tip],
            IkChain::Aim { bones, .. } => bones.iter().map(String::as_str).collect(),
        }
    }

    /// Aim chains solve before limbs (Unity's order: body and look-at first),
    /// so a hand reaching for a foregrip reaches where the aimed torso put it.
    pub fn is_aim(&self) -> bool {
        matches!(self.chain, IkChain::Aim { .. })
    }
}
