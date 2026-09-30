//! src/components/collider/physics_material.rs — per-collider physics material.
//!
//! Unity: `PhysicMaterial` — `dynamicFriction`, `bounciness` and a combine mode
//! for each. Stored **inline** on the collider (#447), not as a shared asset like
//! `MaterialAsset`: a material is four numbers, a scene document stays
//! self-contained, and nothing yet needs "edit one, retune every prop". Promoting
//! it to a shared asset later is additive (an optional asset reference beside the
//! inline values).

use serde::{Deserialize, Serialize};

/// How two touching colliders' coefficients combine into the contact's value
/// (Unity: `PhysicMaterialCombine`). When the two colliders disagree, the mode
/// later in this list wins — `Average < Minimum < Multiply < Maximum`, Unity's
/// priority and rapier's `CoefficientCombineRule` order alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombineMode {
    /// `(a + b) / 2`. The default.
    #[default]
    Average,
    /// `min(a, b)` — e.g. ice keeps its slipperiness against anything.
    Minimum,
    /// `a * b`.
    Multiply,
    /// `max(a, b)` — e.g. a rubber ball bounces off mud.
    Maximum,
}

impl CombineMode {
    /// The Unity-style name the Lua API and inspector show.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Average => "Average",
            Self::Minimum => "Minimum",
            Self::Multiply => "Multiply",
            Self::Maximum => "Maximum",
        }
    }

    /// Parse the Lua/editor name back to the mode, `None` on an unknown string.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }

    /// Every mode, in priority order (the inspector's combo box lists these).
    pub const ALL: [Self; 4] = [Self::Average, Self::Minimum, Self::Multiply, Self::Maximum];
}

/// Surface response of one collider: how much it grips and how much it bounces.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhysicsMaterial {
    /// Coulomb friction coefficient, `>= 0`. `0` is ice; `1` is rubber on concrete.
    pub friction: f32,
    /// Restitution in `[0, 1]`: `0` absorbs an impact, `1` rebounds at full speed.
    pub bounciness: f32,
    pub friction_combine: CombineMode,
    pub bounce_combine: CombineMode,
}

impl PhysicsMaterial {
    /// rapier's own default friction. Kept (over Unity's `0.6`) so every scene
    /// authored before #447 simulates exactly as it did.
    pub const DEFAULT_FRICTION: f32 = 0.5;

    /// A material with the given coefficients clamped into range (friction
    /// `>= 0`, bounciness `[0, 1]`) and the default `Average` combine modes.
    pub fn new(friction: f32, bounciness: f32) -> Self {
        Self {
            friction: friction.max(0.0),
            bounciness: bounciness.clamp(0.0, 1.0),
            ..Self::default()
        }
    }
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            friction: Self::DEFAULT_FRICTION,
            bounciness: 0.0,
            friction_combine: CombineMode::Average,
            bounce_combine: CombineMode::Average,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_mode_names_round_trip() {
        for m in CombineMode::ALL {
            assert_eq!(CombineMode::parse(m.as_str()), Some(m));
        }
        assert_eq!(CombineMode::Minimum.as_str(), "Minimum");
        assert_eq!(CombineMode::parse("Min"), None);
    }

    #[test]
    fn new_clamps_into_range() {
        let m = PhysicsMaterial::new(-2.0, 1.5);
        assert_eq!(m.friction, 0.0);
        assert_eq!(m.bounciness, 1.0);
        let m = PhysicsMaterial::new(0.8, -0.5);
        assert_eq!(m.friction, 0.8);
        assert_eq!(m.bounciness, 0.0);
        assert_eq!(PhysicsMaterial::new(0.3, 0.4).bounciness, 0.4);
    }

    #[test]
    fn default_matches_the_pre_447_rapier_defaults() {
        let m = PhysicsMaterial::default();
        assert_eq!(m.friction, 0.5);
        assert_eq!(m.bounciness, 0.0);
        assert_eq!(m.friction_combine, CombineMode::Average);
        assert_eq!(m.bounce_combine, CombineMode::Average);
    }

    #[test]
    fn missing_fields_deserialize_to_defaults() {
        let m: PhysicsMaterial = serde_json::from_str(r#"{"bounciness":0.7}"#).unwrap();
        assert_eq!(m.bounciness, 0.7);
        assert_eq!(m.friction, 0.5);
        assert_eq!(m.bounce_combine, CombineMode::Average);
    }
}
