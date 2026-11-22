use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IkConstraint {
    /// How scripts address the constraint (`"LeftHand"`, `"Look"`); unique per
    /// animator.
    pub name: String,
    pub chain: IkChain,
    #[serde(default = "full")]
    pub weight: f32,
    #[serde(skip)]
    pub target: Option<IkTarget>,
    #[serde(skip)]
    pub hint: Option<IkTarget>,
    /// `(bone, local rotation written, rotation before)`: what the next solve puts back first.
    #[serde(skip)]
    pub applied: Vec<(u32, Quat, Quat)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IkChain {
    TwoBone {
        root: String,
        mid: String,
        tip: String,
    },
    Aim {
        bones: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        weights: Vec<f32>,
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
