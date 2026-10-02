//! src/components/offmesh_link.rs — OffMeshLink (#462).
//!
//! Unity's `OffMeshLink` / `NavMeshLink`: an authored connection between two
//! walkable points that walking can't join — a ladder between floors, a window to
//! vault, a gap to jump. `start` and `end` are offsets from the entity's Transform
//! (rotation and scale apply), so the link is placed by moving its GameObject. Each
//! end snaps onto the navmesh within [`LINK_SNAP_DISTANCE`]; a link with an end too
//! far from any walkable surface connects nothing.
//!
//! The navmesh's auto-generated drop and jump links (`NavMeshSettings`) share the
//! traversal data here: [`OffMeshLinkData`] is what an agent standing on a link
//! exposes to its script (Unity's `NavMeshAgent.currentOffMeshLinkData`).

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// How far (world units) a link end may lie from the navmesh and still connect.
pub const LINK_SNAP_DISTANCE: f32 = 1.0;

/// Where a link came from (Unity's `OffMeshLinkType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OffMeshLinkKind {
    /// An authored `OffMeshLink` component.
    #[default]
    Manual,
    /// Auto-generated: a drop off a ledge onto the floor below.
    Drop,
    /// Auto-generated: a jump across a gap or up onto a ledge.
    Jump,
}

impl OffMeshLinkKind {
    /// The lowercase name scripts see (`"manual"`, `"drop"`, `"jump"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Drop => "drop",
            Self::Jump => "jump",
        }
    }
}

/// One link as an agent traverses it: its kind, the world points it runs between in
/// the direction of travel, and the entity that authored it (`None` when generated).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffMeshLinkData {
    pub kind: OffMeshLinkKind,
    pub start: Vec3,
    pub end: Vec3,
    pub owner: Option<u32>,
}

/// An authored off-mesh link. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OffMeshLinkComponent {
    /// Unity's `activated`: an inactive link connects nothing.
    pub active: bool,
    /// The start point, local to the entity (the Transform itself by default).
    pub start: Vec3,
    /// The end point, local to the entity.
    pub end: Vec3,
    /// Whether agents may cross it end to start as well (Unity's `biDirectional`).
    pub bidirectional: bool,
    /// The path cost of crossing it, in world units; negative uses its length
    /// (Unity's `costOverride`).
    pub cost_override: f32,
    /// The link's navigation area (#460, Unity's `area`): agents whose area mask
    /// excludes it never cross it, and without a cost override its length is
    /// charged at the area's cost.
    pub area: u8,
}

impl Default for OffMeshLinkComponent {
    /// Unity's defaults: active, both ways, cost from the length; the end 2 m ahead.
    fn default() -> Self {
        Self {
            active: true,
            start: Vec3::ZERO,
            end: Vec3::new(0.0, 0.0, 2.0),
            bidirectional: true,
            cost_override: -1.0,
            area: 0,
        }
    }
}

impl OffMeshLinkComponent {
    /// The link's two ends in world space, given its entity's world matrix.
    pub fn world_ends(&self, world: Mat4) -> (Vec3, Vec3) {
        (
            world.transform_point3(self.start),
            world.transform_point3(self.end),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_follow_the_transform() {
        let link = OffMeshLinkComponent::default();
        let world = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(1.0, 3.0, 0.0),
        );
        let (a, b) = link.world_ends(world);
        assert_eq!(a, Vec3::new(1.0, 3.0, 0.0));
        assert!(b.abs_diff_eq(Vec3::new(5.0, 3.0, 0.0), 1e-5), "{b}");
    }

    #[test]
    fn kind_names_are_lowercase() {
        let names = [
            OffMeshLinkKind::Manual,
            OffMeshLinkKind::Drop,
            OffMeshLinkKind::Jump,
        ]
        .map(OffMeshLinkKind::as_str);
        assert_eq!(names, ["manual", "drop", "jump"]);
    }
}
