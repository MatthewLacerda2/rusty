//! src/navigation/surface.rs — the floor's shape inside a span's cell (#781).
//!
//! A span's `y` is its cell's **highest** walkable point: the right height for
//! connectivity (`links.rs` compares tops), the wrong one to stand an agent on. On a
//! ramp of grade `g` that top sits up to `g × spacing` above the surface under the
//! agent, and it changes in steps from cell to cell.
//!
//! So the bake also keeps, per span, the plane of the triangle that forms its top
//! (Unity's agents follow the detail mesh; this is its one-plane-per-cell form). An
//! agent stands at that plane's height under its XZ, kept between two bounds:
//!
//! - never above the span's top `y`: past a ramp's upper edge the plane would climb
//!   into the air, and the flat deck beyond is the top;
//! - never below `floor`, the top of a lower walkable solid merged into the same one
//!   (the pool floor a ramp's foot rests on): past a ramp's lower edge the plane would
//!   dig under it.
//!
//! A flat span's plane is level with its top, so it reads back exactly `y`.

use glam::Vec2;

use super::{NavigationGraph, SpanRef};

/// A span's floor plane relative to its top `y`, and how far below `y` it may dip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    /// The plane's height at the cell centre, minus the span's `y`.
    pub centre: f32,
    /// The plane's rise per metre along x and z.
    pub slope: Vec2,
    /// How far below the span's `y` the surface reaches at most (`∞` with no floor
    /// under it).
    pub floor: f32,
}

impl Surface {
    /// A level floor at the span's top.
    pub const FLAT: Self = Self {
        centre: 0.0,
        slope: Vec2::ZERO,
        floor: 0.0,
    };

    /// The surface's height, relative to the span's top, `offset` from the cell
    /// centre in XZ.
    pub fn height(&self, offset: Vec2) -> f32 {
        (self.centre + self.slope.dot(offset)).clamp(-self.floor, 0.0)
    }
}

impl NavigationGraph {
    /// The height of span `s`'s floor under world `(x, z)`: what an agent on it
    /// stands on. A point outside the span's cell reads the cell's nearest edge.
    pub fn surface_y(&self, s: SpanRef, x: f32, z: f32) -> f32 {
        let span = &self.spans[s.index as usize];
        let centre = self.cell_center(s.gx, s.gz);
        let half = 0.5 * self.grid_spacing;
        let offset =
            (Vec2::new(x - centre.x, z - centre.z)).clamp(-Vec2::splat(half), Vec2::splat(half));
        span.y + span.surface.height(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 0.3-grade ramp rising along +z whose cell top is at 0.15 (its uphill edge).
    fn ramp() -> Surface {
        Surface {
            centre: -0.15,
            slope: Vec2::new(0.0, 0.3),
            floor: f32::INFINITY,
        }
    }

    #[test]
    fn a_flat_span_reads_its_top() {
        assert_eq!(Surface::FLAT.height(Vec2::new(0.4, -0.3)), 0.0);
    }

    #[test]
    fn a_ramp_follows_its_plane_across_the_cell() {
        let r = ramp();
        assert!((r.height(Vec2::new(0.0, -0.5)) + 0.3).abs() < 1e-6);
        assert!((r.height(Vec2::ZERO) + 0.15).abs() < 1e-6);
        assert!((r.height(Vec2::new(0.3, 0.5))).abs() < 1e-6);
    }

    #[test]
    fn the_plane_stays_between_the_floor_and_the_top() {
        let r = Surface {
            floor: 0.2,
            ..ramp()
        };
        assert_eq!(r.height(Vec2::new(0.0, -0.5)), -0.2, "held up by the floor");
        let up = Surface {
            centre: 0.1,
            ..ramp()
        };
        assert_eq!(up.height(Vec2::new(0.0, 0.5)), 0.0, "never above the top");
    }

    #[test]
    fn surface_y_is_world_height_and_clamps_to_the_cell() {
        let mut g = NavigationGraph::new(0.0, 2.0, 0.0, 2.0, 1.0);
        let r = g.span_range(1, 1);
        g.spans[r.start].y = 1.15;
        g.spans[r.start].surface = ramp();
        let s = SpanRef {
            gx: 1,
            gz: 1,
            index: r.start as u32,
        };
        assert!((g.surface_y(s, 1.0, 0.5) - 0.85).abs() < 1e-6);
        assert!((g.surface_y(s, 1.0, -3.0) - 0.85).abs() < 1e-6, "clamped");
        assert!((g.surface_y(s, 1.2, 1.25) - 1.075).abs() < 1e-6);
    }
}
