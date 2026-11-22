use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::{EntityRefs, RefVisit};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum JointKind {
        #[default]
        Fixed = "Fixed",
        Hinge = "Hinge",
        Ball = "Ball",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JointComponent {
    pub kind: JointKind,
    pub connected_body: Option<u32>,
    pub anchor: Vec3,
    pub connected_anchor: Vec3,
    pub auto_configure_connected_anchor: bool,
    pub axis: Vec3,
    pub swing_axis: Vec3,
    pub use_limits: bool,
    pub limits: Vec2,
    pub swing_limit: f32,
    pub swing2_limit: f32,
    pub break_force: f32,
    pub break_torque: f32,
    pub enable_collision: bool,
    pub drive_strength: f32,
    pub drive_damping: f32,
}

impl Default for JointComponent {
    fn default() -> Self {
        Self {
            kind: JointKind::Fixed,
            connected_body: None,
            anchor: Vec3::ZERO,
            connected_anchor: Vec3::ZERO,
            auto_configure_connected_anchor: true,
            axis: Vec3::X,
            swing_axis: Vec3::Y,
            use_limits: false,
            limits: Vec2::new(-45.0, 45.0),
            swing_limit: 45.0,
            swing2_limit: 45.0,
            break_force: 0.0,
            break_torque: 0.0,
            enable_collision: false,
            drive_strength: 0.0,
            drive_damping: 0.0,
        }
    }
}

impl JointComponent {
    pub const REF_POINTERS: [&'static str; 1] = ["/connected_body"];
}

impl EntityRefs for JointComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        visit(Self::REF_POINTERS[0], &mut self.connected_body);
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        if let Some(r) = self.connected_body {
            visit(Self::REF_POINTERS[0], r);
        }
    }

    #[doc(hidden)]
    fn every_ref() -> Self {
        Self::default()
    }
}

impl crate::FirstClass for JointComponent {
    const NAME: &'static str = "Joint";
    const ALIASES: &'static [&'static str] = &["fixedjoint", "hingejoint", "characterjoint"];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_rewrites_and_drops_the_connected_body() {
        let mut j = JointComponent {
            connected_body: Some(4),
            ..Default::default()
        };
        j.remap_refs(&|id| Some(id + 10));
        assert_eq!(j.connected_body, Some(14));
        j.remap_refs(&|_| None);
        assert_eq!(j.connected_body, None);
    }

    #[test]
    fn a_sparse_joint_fills_the_defaults_and_round_trips() {
        let sparse: JointComponent =
            serde_json::from_str(r#"{"kind":"Ball","swing_limit":30.0}"#).unwrap();
        let d = JointComponent::default();
        assert_eq!(
            (sparse.swing_limit, sparse.swing2_limit),
            (30.0, d.swing2_limit)
        );
        assert_eq!(sparse.swing_axis, Vec3::Y);
        let new = JointComponent {
            swing2_limit: 70.0,
            ..sparse
        };
        let json = serde_json::to_string(&new).unwrap();
        assert_eq!(serde_json::from_str::<JointComponent>(&json).unwrap(), new);
    }
}
