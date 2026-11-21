use bytemuck::Zeroable;
use glam::{Vec2, Vec4};

use rusty_components::{GradientKind, UiGradient};

pub(crate) const MODE_IMAGE: f32 = 0.0;
pub(crate) const MODE_TEXT: f32 = 1.0;
pub(crate) const MODE_SHAPE: f32 = 2.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct UiVertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) glow: [f32; 4],
    pub(crate) sdf: [f32; 4],
    pub(crate) local: [f32; 4],
    pub(crate) shape: [f32; 4],
    pub(crate) radii: [f32; 4],
    pub(crate) grad: [f32; 4],
    pub(crate) stop1: [f32; 4],
    pub(crate) stop2: [f32; 4],
    pub(crate) stop3: [f32; 4],
    pub(crate) stop_t: [f32; 4],
}

pub(crate) fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBS: [wgpu::VertexAttribute; 14] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4,
        9 => Float32x4,
        10 => Float32x4,
        11 => Float32x4,
        12 => Float32x4,
        13 => Float32x4,
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Fill {
    color: [f32; 4],
    grad: [f32; 4],
    stops: [[f32; 4]; 3],
    stop_t: [f32; 4],
}

impl Fill {
    pub(crate) fn new(gradient: Option<&UiGradient>, tint: Vec4) -> Self {
        let Some(g) = gradient.filter(|g| !g.stops.is_empty()) else {
            return Self {
                color: tint.to_array(),
                ..Default::default()
            };
        };
        let last = g.stops[g.stops.len() - 1];
        let stop = |i: usize| g.stops.get(i).map_or((1.0, last.color), |s| (s.t, s.color));
        let [s0, s1, s2, s3] = [0, 1, 2, 3].map(stop);
        let grad = match g.kind {
            GradientKind::Linear => {
                let a = g.angle.to_radians();
                let dir = Vec2::new(a.cos(), a.sin());
                let span = 0.5 * (dir.x.abs() + dir.y.abs());
                [1.0, dir.x, dir.y, span.max(1e-4)]
            }
            GradientKind::Radial => [2.0, g.center.x, g.center.y, g.radius.max(1e-3)],
        };
        Self {
            color: (s0.1 * tint).to_array(),
            grad,
            stops: [s1.1, s2.1, s3.1].map(|c| (c * tint).to_array()),
            stop_t: [s0.0, s1.0, s2.0, s3.0],
        }
    }

    pub(crate) fn max_alpha(&self) -> f32 {
        let stops = self.stops.iter().map(|s| s[3]);
        stops.fold(self.color[3], f32::max)
    }

    pub(crate) fn vertex(&self, pos: [f32; 2], uv: [f32; 2], local: [f32; 4]) -> UiVertex {
        UiVertex {
            pos,
            uv,
            local,
            color: self.color,
            grad: self.grad,
            stop1: self.stops[0],
            stop2: self.stops[1],
            stop3: self.stops[2],
            stop_t: self.stop_t,
            ..Zeroable::zeroed()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_components::GradientStop;

    #[test]
    fn a_flat_fill_has_no_gradient_and_a_gradient_pads_to_four_stops() {
        let flat = Fill::new(None, Vec4::new(1.0, 0.0, 0.0, 0.5));
        assert_eq!(flat.grad[0], 0.0);
        assert_eq!(flat.max_alpha(), 0.5);
        let g = UiGradient {
            angle: 90.0,
            stops: vec![
                GradientStop {
                    t: 0.25,
                    color: Vec4::new(1.0, 0.0, 0.0, 1.0),
                },
                GradientStop {
                    t: 0.75,
                    color: Vec4::new(0.0, 0.0, 1.0, 1.0),
                },
            ],
            ..Default::default()
        };
        let fill = Fill::new(Some(&g), Vec4::new(1.0, 1.0, 1.0, 0.5));
        assert_eq!(fill.grad[0], 1.0);
        assert!((fill.grad[2] - 1.0).abs() < 1e-6, "90° points up");
        assert_eq!(fill.stop_t, [0.25, 0.75, 1.0, 1.0]);
        assert_eq!(
            fill.stops[2],
            [0.0, 0.0, 1.0, 0.5],
            "padded with the last, tinted"
        );
        assert_eq!(fill.color, [1.0, 0.0, 0.0, 0.5]);
    }
}
