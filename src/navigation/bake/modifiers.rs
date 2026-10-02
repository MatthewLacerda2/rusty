//! src/navigation/bake/modifiers.rs — `NavMeshModifierVolume`s assign areas (#460).
//!
//! A modifier volume is a box at a world pose. After carving, every pre-erosion
//! span whose floor point (its cell centre at its floor height) lies inside an
//! active volume takes that volume's area; a span inside a `NotWalkable` volume is
//! removed, so erosion then pulls the surface back from it as from a wall. Where
//! volumes overlap, `NotWalkable` wins, then the highest area id (Unity's rule), so
//! the result never depends on the order volumes are visited in.
//!
//! Like carving, this is per column, so a moved, resized or retargeted volume
//! dirties only the cells its old and new boxes cover (`inputs`), and an
//! incremental rebake there matches a full bake.

use glam::{Mat4, Vec3};

use super::super::{NavSpan, NavigationGraph, NOT_WALKABLE_AREA, WALKABLE_AREA};
use super::columns::Columns;
use super::raster;
use super::region::CellRect;
use crate::scene::Scene;

/// How far outside its box a floor may sit and still count as inside: absorbs float
/// error on a floor lying exactly on the box's face.
const INSIDE_TOLERANCE: f32 = 1e-3;

/// A volume as the bake reads it: its box, its area and its world pose.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ModifierVolume {
    pub center: Vec3,
    pub size: Vec3,
    pub area: u8,
    pub pose: Mat4,
}

impl ModifierVolume {
    /// The box's eight corners in world space.
    fn corners(&self) -> [Vec3; 8] {
        let h = self.size * 0.5;
        std::array::from_fn(|i| {
            let sign = |bit: usize| if i & bit == 0 { -1.0 } else { 1.0 };
            let local = self.center + Vec3::new(h.x * sign(1), h.y * sign(2), h.z * sign(4));
            self.pose.transform_point3(local)
        })
    }

    /// The cells the box's XZ extent covers.
    pub fn footprint(&self, g: &NavigationGraph) -> Option<CellRect> {
        raster::footprint(g, &self.corners())
    }

    /// Whether world point `p` lies inside the box. A degenerate pose (zero scale)
    /// holds nothing.
    fn contains(&self, inverse: &Mat4, p: Vec3) -> bool {
        let d = (inverse.transform_point3(p) - self.center).abs();
        let h = self.size.abs() * 0.5 + Vec3::splat(INSIDE_TOLERANCE);
        d.cmple(h).all()
    }
}

/// The active volumes on active entities, by ascending id.
pub(super) fn modifier_keys(scene: &Scene) -> Vec<(u32, ModifierVolume)> {
    let mut out: Vec<_> = scene
        .world
        .ids_with_nav_modifier()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .filter_map(|id| {
            let m = scene.world.nav_modifier(id)?.clone();
            m.active.then_some(())?;
            let pose = scene.compute_world_matrix(id);
            Some((
                id,
                ModifierVolume {
                    center: m.center,
                    size: m.size,
                    area: m.area,
                    pose,
                },
            ))
        })
        .collect();
    out.sort_by_key(|&(id, _)| id);
    out
}

/// Which of two areas wins where volumes overlap.
fn stronger(a: u8, b: u8) -> u8 {
    if a == NOT_WALKABLE_AREA || b == NOT_WALKABLE_AREA {
        NOT_WALKABLE_AREA
    } else {
        a.max(b)
    }
}

/// Assign the `volumes`' areas to the spans of `cols` (the columns of `region`),
/// dropping the spans a `NotWalkable` volume holds.
pub(super) fn apply(
    g: &NavigationGraph,
    cols: &mut Columns,
    region: CellRect,
    volumes: &[&ModifierVolume],
) {
    let boxes: Vec<_> = volumes
        .iter()
        .filter(|v| v.pose.determinant().abs() > f32::EPSILON)
        .map(|v| (*v, v.pose.inverse(), v.footprint(g)))
        .collect();
    if boxes.is_empty() {
        return;
    }
    let mut out = Columns::with_capacity(region.cells());
    let mut local = 0;
    for gz in region.z0..=region.z1 {
        for gx in region.x0..=region.x1 {
            let here: Vec<_> = boxes
                .iter()
                .filter(|(_, _, r)| r.is_some_and(|r| r.contains(gx, gz)))
                .collect();
            let centre = g.cell_center(gx, gz);
            let assign = |s: &NavSpan| {
                let p = Vec3::new(centre.x, s.y, centre.z);
                let area = here
                    .iter()
                    .filter(|(v, inv, _)| v.contains(inv, p))
                    .map(|(v, _, _)| v.area)
                    .reduce(stronger)?;
                Some(NavSpan { area, ..*s })
            };
            let column = cols.column(local).iter().filter_map(|s| match assign(s) {
                Some(s) if s.area == NOT_WALKABLE_AREA => None,
                Some(s) => Some(s),
                None => Some(NavSpan {
                    area: WALKABLE_AREA,
                    ..*s
                }),
            });
            out.push_column(column.collect::<Vec<_>>());
            local += 1;
        }
    }
    *cols = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_walkable_beats_every_area_then_the_highest_id_wins() {
        assert_eq!(stronger(5, NOT_WALKABLE_AREA), NOT_WALKABLE_AREA);
        assert_eq!(stronger(3, 7), 7);
        assert_eq!(stronger(WALKABLE_AREA, 2), 2);
    }

    #[test]
    fn the_box_turns_and_scales_with_its_pose() {
        let v = ModifierVolume {
            center: Vec3::ZERO,
            size: Vec3::new(4.0, 2.0, 1.0),
            area: 3,
            pose: Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        let inv = v.pose.inverse();
        assert!(
            v.contains(&inv, Vec3::new(0.0, 0.0, 1.9)),
            "long axis now on z"
        );
        assert!(!v.contains(&inv, Vec3::new(1.9, 0.0, 0.0)));
        assert!(
            v.contains(&inv, Vec3::new(0.0, 1.0, 0.0)),
            "the top face holds"
        );
    }
}
