use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CombineMode {
        #[default]
        Average = "Average",
        Minimum = "Minimum",
        Multiply = "Multiply",
        Maximum = "Maximum",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhysicsMaterial {
    pub friction: f32,
    pub bounciness: f32,
    pub friction_combine: CombineMode,
    pub bounce_combine: CombineMode,
}

impl PhysicsMaterial {
    /// rapier's own default, kept over Unity's 0.6: the physics tests and tuned scenes rest on it.
    const DEFAULT_FRICTION: f32 = 0.5;

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
