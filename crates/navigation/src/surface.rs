//! A span's `y` is its cell's **highest** point, right for connectivity but wrong to stand on: on a
//! ramp of grade `g` it sits up to `g × spacing` above the surface. The bake keeps the plane of the
//! top triangle (Unity follows the detail mesh; this is one plane per cell). An agent stands at its
//! height, never above the span's `y` and never below `floor`, the top of a lower solid merged into
//! the span (the pool floor a ramp's foot rests on).

use glam::Vec2;

use super::{NavigationGraph, SpanRef};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub centre: f32,
    pub slope: Vec2,
    /// `∞` with no floor under it.
    pub floor: f32,
}

impl Surface {
    pub const FLAT: Self = Self {
        centre: 0.0,
        slope: Vec2::ZERO,
        floor: 0.0,
    };

    pub fn height(&self, offset: Vec2) -> f32 {
        (self.centre + self.slope.dot(offset)).clamp(-self.floor, 0.0)
    }
}

impl NavigationGraph {
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

    #[test]
    fn surface_y_clamps_to_half_the_spacing() {
        let mut g = NavigationGraph::new(0.0, 4.0, 0.0, 4.0, 2.0);
        let r = g.span_range(1, 1);
        g.spans[r.start].surface = ramp();
        let s = SpanRef {
            gx: 1,
            gz: 1,
            index: r.start as u32,
        };
        assert!((g.surface_y(s, 2.0, 1.1) + 0.42).abs() < 1e-6);
        assert!((g.surface_y(s, 2.0, -1.0) + 0.45).abs() < 1e-6, "clamped");
    }
}
