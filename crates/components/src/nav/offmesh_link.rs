use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// How far in metres a link end may lie from the navmesh and still connect.
pub const LINK_SNAP_DISTANCE: f32 = 1.0;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum OffMeshLinkKind {
        #[default]
        Manual = "manual",
        Drop = "drop",
        Jump = "jump",
    }
}

/// A link as an agent crosses it, ends in travel order; `owner` is `None` for a generated link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffMeshLinkData {
    pub kind: OffMeshLinkKind,
    pub start: Vec3,
    pub end: Vec3,
    pub owner: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OffMeshLinkComponent {
    pub active: bool,
    pub start: Vec3,
    pub end: Vec3,
    pub bidirectional: bool,
    pub cost_override: f32,
    pub area: u8,
}

impl Default for OffMeshLinkComponent {
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
    pub fn world_ends(&self, world: Mat4) -> (Vec3, Vec3) {
        (
            world.transform_point3(self.start),
            world.transform_point3(self.end),
        )
    }
}

impl crate::FirstClass for OffMeshLinkComponent {
    const NAME: &'static str = "OffMeshLink";
    const ALIASES: &'static [&'static str] = &["navmeshlink"];
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
        .map(OffMeshLinkKind::name);
        assert_eq!(names, ["manual", "drop", "jump"]);
    }
}
