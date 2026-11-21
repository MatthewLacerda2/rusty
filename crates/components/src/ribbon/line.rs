use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use super::RibbonStyle;

/// Authoring component: a ribbon through `positions` (Unity's `LineRenderer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineComponent {
    /// World space, or the entity's local space when `use_world_space` is off.
    pub positions: Vec<Vec3>,
    pub use_world_space: bool,
    #[serde(default)]
    pub looping: bool,
    pub style: RibbonStyle,
}

impl Default for LineComponent {
    fn default() -> Self {
        Self {
            positions: vec![Vec3::ZERO, Vec3::Z],
            use_world_space: true,
            looping: false,
            style: RibbonStyle::default(),
        }
    }
}

impl LineComponent {
    pub fn world_positions(&self, world: Mat4) -> Vec<Vec3> {
        if self.use_world_space {
            return self.positions.clone();
        }
        let points = self.positions.iter();
        points.map(|&p| world.transform_point3(p)).collect()
    }
}

/// Why a gradient's dropped key list is not the component's default.
const GRADIENT_LIST: &str = "a gradient written without this list reads it as `Gradient` does everywhere (no alpha keys opaque, no colour keys white), not as the component's default gradient";

impl crate::FirstClass for LineComponent {
    const NAME: &'static str = "Line";
    const ALIASES: &'static [&'static str] = &["linerenderer"];
    const INVENTORY: &'static str = "LineRenderer";
    const DEFAULT_EXCEPTIONS: &'static [(&'static str, &'static str)] = &[
        ("style.color.alpha", GRADIENT_LIST),
        ("style.color.color_keys", GRADIENT_LIST),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_space_follows_the_entity() {
        let mut line = LineComponent::default();
        let world = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(line.world_positions(world), vec![Vec3::ZERO, Vec3::Z]);
        line.use_world_space = false;
        let moved = line.world_positions(world);
        assert_eq!(
            moved,
            vec![Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, 4.0)]
        );
    }
}
