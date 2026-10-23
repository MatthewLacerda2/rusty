//! src/components/joint.rs — Joint: a constraint between two rigid bodies (#449).
//!
//! Unity's `FixedJoint`, `HingeJoint` and `CharacterJoint`, folded into one
//! component with a [`JointKind`]. It ties its entity's Rigidbody to the
//! `connected_body`'s — or, with none, to a fixed point in the world:
//!
//! - `Fixed` welds the two bodies (breakable debris, a gun in a ragdoll's hand);
//! - `Hinge` leaves one rotation free, about `axis`, optionally within
//!   `limits` (doors, elbows, knees);
//! - `Ball` leaves all three rotations free: twist about `axis` within `limits`,
//!   a swing of up to `swing_limit` degrees about `swing_axis` (Unity's swing 1)
//!   and of up to `swing2_limit` about `axis × swing_axis` (swing 2) — so a
//!   shoulder can swing far forward yet little sideways (shoulders, hips, a
//!   hanging lamp).
//!
//! `anchor`, `axis` and `swing_axis` are in this entity's local space. Under a
//! negative world scale (the entity's own or a parent's), the joint is the
//! mirror image of the unmirrored one, twist and hinge limits acting on the
//! mirror-image side (#1125). `connected_anchor` is in
//! the connected body's local space — or world space with no connected body —
//! and is ignored while `auto_configure_connected_anchor` is on, which puts it
//! wherever `anchor` sits when the joint is built (Unity's default). The joint's
//! rest pose is the pose the two bodies are in when it is built. A non-zero
//! `break_force` / `break_torque` breaks the joint when exceeded (see
//! `physics::joints`). The one entity reference, `connected_body`, follows the
//! entity through prefab save / stamp ([`JointComponent::remap_refs`]).

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::components::{EntityRefs, RefVisit};

/// Which rotations a joint leaves free. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointKind {
    /// No relative motion at all.
    #[default]
    Fixed,
    /// One free rotation, about the joint axis.
    Hinge,
    /// Three free rotations: twist about the axis plus a swing cone.
    Ball,
}

impl JointKind {
    /// Every kind, in menu order.
    pub const ALL: [JointKind; 3] = [Self::Fixed, Self::Hinge, Self::Ball];

    /// The kind's name as scripts and the snapshot spell it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fixed => "Fixed",
            Self::Hinge => "Hinge",
            Self::Ball => "Ball",
        }
    }

    /// Parse a kind name (case-insensitive).
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(name))
    }
}

/// A joint between this entity's Rigidbody and another's. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
// Every field defaults, so a hand-written scene can name only what it sets.
#[serde(default)]
pub struct JointComponent {
    pub kind: JointKind,
    /// The entity whose body this one is joined to; `None` joins it to the world.
    pub connected_body: Option<u32>,
    /// The joint's pivot, in this entity's local space.
    pub anchor: Vec3,
    /// The pivot on the connected body (its local space), or in world space with
    /// no connected body. Ignored while `auto_configure_connected_anchor` is on.
    pub connected_anchor: Vec3,
    /// Derive `connected_anchor` from `anchor` when the joint is built.
    pub auto_configure_connected_anchor: bool,
    /// Hinge axis / Ball twist axis, in this entity's local space.
    pub axis: Vec3,
    /// Ball: the swing-1 axis, in this entity's local space (Unity's
    /// `swingAxis`). Only its part perpendicular to `axis` counts.
    pub swing_axis: Vec3,
    /// Whether `limits` (and a Ball's swing limits) apply.
    pub use_limits: bool,
    /// Hinge angle / Ball twist range, in degrees from the rest pose: `(min, max)`.
    pub limits: Vec2,
    /// Ball: how far (degrees) the body may swing about `swing_axis` (swing 1).
    pub swing_limit: f32,
    /// Ball: how far (degrees) the body may swing about `axis × swing_axis`
    /// (swing 2).
    pub swing2_limit: f32,
    /// Force (newtons) past which the joint breaks; `0` never breaks.
    pub break_force: f32,
    /// Torque (newton-metres) past which the joint breaks; `0` never breaks.
    pub break_torque: f32,
    /// Whether the two joined bodies still collide with each other.
    pub enable_collision: bool,
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
        }
    }
}

impl JointComponent {
    /// The JSON pointer (inside the component) of its entity reference — how
    /// prefab write-back finds the override leaf holding an entity id.
    pub const REF_POINTERS: [&'static str; 1] = ["/connected_body"];
}

/// A dropped `connected_body` joins the body to the world.
impl EntityRefs for JointComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        visit(Self::REF_POINTERS[0], &mut self.connected_body);
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        if let Some(r) = self.connected_body {
            visit(Self::REF_POINTERS[0], r);
        }
    }

    #[cfg(test)]
    fn every_ref() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_by_name() {
        for k in JointKind::ALL {
            assert_eq!(JointKind::parse(&k.name().to_uppercase()), Some(k));
        }
        assert_eq!(JointKind::parse("slider"), None);
    }

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
