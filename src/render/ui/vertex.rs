//! src/render/ui/vertex.rs — the UI vertex format and how a graphic's look rides on it.
//!
//! Every UI graphic — image, glyph, shape — is triangles of one [`UiVertex`], so a
//! HUD of mixed graphics still batches. The vertex carries what the fragment shader
//! (`ui.wgsl`) needs to draw any of them: the tint (or a gradient's 2–4 stops,
//! #425), the rect-local position the gradient and a shape's distance field are
//! evaluated at, and the per-kind SDF parameters. Unused fields are zero.

use bytemuck::Zeroable;
use glam::{Vec2, Vec4};

use crate::components::{GradientKind, UiGradient};

/// What a vertex draws (`sdf[0]`).
pub(crate) const MODE_IMAGE: f32 = 0.0;
/// An SDF glyph (#419).
pub(crate) const MODE_TEXT: f32 = 1.0;
/// An SDF shape (#425).
pub(crate) const MODE_SHAPE: f32 = 2.0;

/// One UI vertex: NDC position, texture coordinate (v down, as the GPU samples) and
/// the straight-alpha display-space tint (group alpha folded in). Text vertices
/// also carry their outline and glow colours and the SDF parameters
/// (`[1, dilate, outline, glow]` in atlas pixels); a shape's are
/// `[2, border width, softness, glow reach]` with its border colour in `outline`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct UiVertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) glow: [f32; 4],
    pub(crate) sdf: [f32; 4],
    /// Rect-local position (reference units from the rect's centre, y up) and the
    /// rect's half size.
    pub(crate) local: [f32; 4],
    /// Shape kind code and its three parameters (see `render::ui::shape`).
    pub(crate) shape: [f32; 4],
    /// A `Rect` shape's per-corner size (top-left, top-right, bottom-right, bottom-left).
    pub(crate) radii: [f32; 4],
    /// The gradient: `[kind (0 none, 1 linear, 2 radial), a, b, c]` — linear: the
    /// unit direction and its corner-to-corner half-span; radial: centre and radius.
    pub(crate) grad: [f32; 4],
    /// Gradient stops 1–3's colours (stop 0's is `color`).
    pub(crate) stop1: [f32; 4],
    pub(crate) stop2: [f32; 4],
    pub(crate) stop3: [f32; 4],
    /// The four stops' positions.
    pub(crate) stop_t: [f32; 4],
}

/// The vertex buffer layout matching `ui.wgsl`'s `VertexIn`.
pub(crate) fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBS: [wgpu::VertexAttribute; 14] = wgpu::vertex_attr_array![
        0 => Float32x2, // pos (NDC)
        1 => Float32x2, // uv
        2 => Float32x4, // color (gradient stop 0)
        3 => Float32x4, // outline colour (text) / border colour (shape)
        4 => Float32x4, // glow colour (text)
        5 => Float32x4, // sdf: [mode, …]
        6 => Float32x4, // local position + half size
        7 => Float32x4, // shape kind + params
        8 => Float32x4, // shape corner radii
        9 => Float32x4, // gradient
        10 => Float32x4, // stop 1
        11 => Float32x4, // stop 2
        12 => Float32x4, // stop 3
        13 => Float32x4, // stop positions
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

/// A graphic's fill as vertex fields: a flat `tint`, or `gradient` with every stop
/// multiplied by `tint` (the group alpha and a Selectable's colour).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Fill {
    color: [f32; 4],
    grad: [f32; 4],
    stops: [[f32; 4]; 3],
    stop_t: [f32; 4],
}

impl Fill {
    /// The fill of `tint`, swept by `gradient` when it has one.
    pub(crate) fn new(gradient: Option<&UiGradient>, tint: Vec4) -> Self {
        let Some(g) = gradient.filter(|g| !g.stops.is_empty()) else {
            return Self {
                color: tint.to_array(),
                ..Default::default()
            };
        };
        // Pad to four stops by repeating the last at t = 1, so the shader always
        // walks four.
        let last = g.stops[g.stops.len() - 1];
        let stop = |i: usize| g.stops.get(i).map_or((1.0, last.color), |s| (s.t, s.color));
        let [s0, s1, s2, s3] = [0, 1, 2, 3].map(stop);
        let grad = match g.kind {
            GradientKind::Linear => {
                let a = g.angle.to_radians();
                let dir = Vec2::new(a.cos(), a.sin());
                // The half-span reaching the furthest corner of a unit square.
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

    /// The most opaque this fill gets.
    pub(crate) fn max_alpha(&self) -> f32 {
        let stops = self.stops.iter().map(|s| s[3]);
        stops.fold(self.color[3], f32::max)
    }

    /// A vertex at `pos` / `uv` / `local` carrying this fill (everything else zero).
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
    use crate::components::GradientStop;

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
