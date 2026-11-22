//! Merged solids whose tops lie within `max_step` of each other OR their walkable flags (Recast's
//! flag-merge threshold). Past Recast: when the solid reaching the merged top carries a
//! `NavMeshModifier` override, its flag and area decide alone, else a sofa overridden to
//! `NotWalkable` whose seat is within `max_step` of the floor would inherit the floor's walkable
//! flag.

use super::super::{NavSpan, NavigationGraph};
use super::columns::Columns;
use super::plane::{top_plane, Plane, TOP_EPS};
use super::raster::Solid;
use super::region::CellRect;

const TOUCH_EPS: f32 = 1e-3;

impl NavigationGraph {
    pub(super) fn build_columns(&self, mut solids: Vec<Solid>, region: CellRect) -> Columns {
        solids.sort_by(|a, b| {
            a.cell
                .cmp(&b.cell)
                .then(a.min.total_cmp(&b.min))
                .then(a.max.total_cmp(&b.max))
                .then(a.walkable.cmp(&b.walkable))
                .then(a.area.cmp(&b.area))
                .then(a.overridden.cmp(&b.overridden))
                .then(Plane::order(a.plane, b.plane))
                .then(a.floor.total_cmp(&b.floor))
        });
        let mut out = Columns::with_capacity(region.cells());
        let mut next = 0;
        let mut column = Vec::new();
        for gz in region.z0..=region.z1 {
            for gx in region.x0..=region.x1 {
                let cell = self.index(gx, gz) as u32;
                let end = next + solids[next..].partition_point(|s| s.cell == cell);
                merge_column(&solids[next..end], self.max_step, &mut column);
                out.push_column(open_spans(&column, self.agent_height));
                next = end;
            }
        }
        out
    }
}

fn open_spans(column: &[Solid], agent_height: f32) -> impl Iterator<Item = NavSpan> + '_ {
    column.iter().enumerate().filter_map(move |(i, s)| {
        let ceiling = column.get(i + 1).map_or(f32::INFINITY, |above| above.min);
        (s.walkable && ceiling - s.max >= agent_height).then_some(NavSpan {
            y: s.max,
            ceiling,
            area: s.area,
            surface: Plane::surface(s.plane, s.max, s.floor),
        })
    })
}

fn merge_column(solids: &[Solid], flag_merge: f32, out: &mut Vec<Solid>) {
    out.clear();
    for &s in solids {
        match out.last_mut() {
            Some(last) if s.min <= last.max + TOUCH_EPS => {
                let top = last.max.max(s.max);
                last.plane = top_plane((last.max, last.plane), (s.max, s.plane));
                last.floor = last.floor.max(s.floor).max(lower_floor(last, &s));
                let authored = (authors(last, top), authors(&s, top));
                let (keeps, takes) = match authored {
                    (false, false) => (
                        last.walkable && last.max >= top - flag_merge,
                        s.walkable && s.max >= top - flag_merge,
                    ),
                    (a, b) => (a && last.walkable, b && s.walkable),
                };
                last.area = match authored {
                    (false, true) if !takes => s.area,
                    _ => merged_area((last, keeps), (&s, takes)),
                };
                last.overridden = authored.0 || authored.1;
                last.walkable = keeps || takes;
                last.max = top;
            }
            _ => out.push(s),
        }
    }
}

fn authors(s: &Solid, top: f32) -> bool {
    s.overridden && s.max >= top - TOP_EPS
}

/// The area of two merging solids' top: that of the walkable one reaching it, the
/// higher one's when both do (the higher id when level, so the visit order never
/// decides).
fn merged_area((a, a_walks): (&Solid, bool), (b, b_walks): (&Solid, bool)) -> u8 {
    match (a_walks, b_walks) {
        (true, true) if a.max > b.max + TOP_EPS => a.area,
        (true, true) if b.max > a.max + TOP_EPS => b.area,
        (true, true) => a.area.max(b.area),
        (false, true) => b.area,
        _ => a.area,
    }
}

fn lower_floor(a: &Solid, b: &Solid) -> f32 {
    let (low, high) = if a.max <= b.max { (a, b) } else { (b, a) };
    if low.walkable && low.plane.is_some() && low.max < high.max - TOP_EPS {
        low.max
    } else {
        f32::NEG_INFINITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(min: f32, max: f32, walkable: bool) -> Solid {
        Solid {
            cell: 0,
            min,
            max,
            walkable,
            plane: None,
            floor: f32::NEG_INFINITY,
            area: 0,
            overridden: false,
        }
    }

    #[test]
    fn touching_solids_merge_and_the_higher_top_decides() {
        let mut out = Vec::new();
        let floor = solid(-0.1, 0.0, true);
        let crate_ = solid(0.0, 1.0, false);
        merge_column(&[floor, crate_], 0.5, &mut out);
        assert_eq!(out, vec![solid(-0.1, 1.0, false)]);
    }

    #[test]
    fn a_low_unwalkable_lip_keeps_the_floor_walkable() {
        let mut out = Vec::new();
        merge_column(
            &[solid(-0.1, 0.0, true), solid(-0.1, 0.2, false)],
            0.5,
            &mut out,
        );
        assert!(
            out[0].walkable,
            "a 0.2 lip is within the flag-merge threshold"
        );
        assert_eq!(out[0].max, 0.2);
    }

    #[test]
    fn a_floor_must_lie_more_than_top_eps_below() {
        let flat = Some(Plane {
            y: 0.0,
            slope: glam::Vec2::ZERO,
        });
        let low = Solid {
            plane: flat,
            ..solid(-0.1, 0.0, true)
        };
        let level = solid(-0.1, TOP_EPS, true);
        assert_eq!(lower_floor(&low, &level), f32::NEG_INFINITY, "level tops");
        let above = solid(-0.1, 2.0 * TOP_EPS, true);
        assert_eq!(lower_floor(&above, &low), 0.0, "either order");
    }

    #[test]
    fn a_gap_keeps_two_solids() {
        let mut out = Vec::new();
        merge_column(
            &[solid(-0.1, 0.0, true), solid(3.0, 3.2, true)],
            0.5,
            &mut out,
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn an_overridden_top_skips_the_flag_merge() {
        let mut out = Vec::new();
        let seat = |area| Solid {
            area,
            overridden: true,
            ..solid(0.0, 0.45, area != 1)
        };
        merge_column(&[solid(-0.1, 0.0, true), seat(1)], 0.5, &mut out);
        assert!(!out[0].walkable, "a NotWalkable seat stays unwalkable");
        assert_eq!((out[0].area, out[0].max), (1, 0.45));
        merge_column(&[solid(-0.1, 0.0, true), seat(4)], 0.5, &mut out);
        assert!(
            out[0].walkable && out[0].area == 4,
            "another area keeps its id"
        );
        let rug = Solid {
            overridden: true,
            ..solid(-0.1, -0.05, false)
        };
        merge_column(&[rug, solid(-0.1, 0.0, true)], 0.5, &mut out);
        assert!(out[0].walkable, "an override below the top decides nothing");
    }
}
