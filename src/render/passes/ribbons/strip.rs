//! src/render/passes/ribbons/strip.rs — a ribbon's centre line → strip geometry.
//!
//! Pure CPU and GPU-free, so it is unit-tested directly. Each point becomes a pair
//! of vertices (`side` −1 / +1) carrying the centre, the tangent, and the width,
//! colour and texture `u` the style gives that point's place along the length;
//! the shader turns the pair to face the camera. Consecutive coincident points
//! are merged first (a zero-length segment has no direction).

use glam::Vec3;

use crate::components::{RibbonStyle, TextureMode};

/// One strip vertex (matches `ribbons.wgsl`'s `VertexInput`).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RibbonVertex {
    pub(crate) center: [f32; 3],
    pub(crate) side: f32,
    pub(crate) tangent: [f32; 3],
    pub(crate) width: f32,
    pub(crate) color: [f32; 4],
    pub(crate) u: f32,
}

impl RibbonVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x3, // center
        1 => Float32,   // side
        2 => Float32x3, // tangent
        3 => Float32,   // width
        4 => Float32x4, // color
        5 => Float32,   // u
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RibbonVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Points closer than this are one point.
const MERGE: f32 = 1e-5;

/// Append the strip for `points` (world space) to `vertices` / `indices`.
/// Returns the number of indices added — `0` when fewer than two distinct points
/// remain. A `looping` strip closes back onto its first point.
pub(crate) fn build(
    points: &[Vec3],
    looping: bool,
    style: &RibbonStyle,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) -> u32 {
    let mut line = merged(points);
    if line.len() < 2 {
        return 0;
    }
    let closed = looping && line.len() > 2;
    if closed {
        line.push(line[0]);
    }
    let lengths = cumulative(&line);
    let total = lengths[lengths.len() - 1];
    let base = vertices.len() as u32;
    for (i, &center) in line.iter().enumerate() {
        let t = if total > 0.0 { lengths[i] / total } else { 0.0 };
        let u = match style.texture_mode {
            TextureMode::Stretch => t,
            TextureMode::Tile => lengths[i],
        };
        let tangent = tangent_at(&line, i, closed);
        for side in [-1.0, 1.0] {
            vertices.push(RibbonVertex {
                center: center.to_array(),
                side,
                tangent: tangent.to_array(),
                width: style.width.evaluate(t),
                color: style.color.evaluate(t),
                u,
            });
        }
    }
    let segments = line.len() as u32 - 1;
    for s in 0..segments {
        let (a, b, c, d) = (
            base + 2 * s,
            base + 2 * s + 1,
            base + 2 * s + 2,
            base + 2 * s + 3,
        );
        indices.extend_from_slice(&[a, b, c, b, d, c]);
    }
    segments * 6
}

/// `points` with consecutive near-duplicates (and non-finite points) dropped.
fn merged(points: &[Vec3]) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::with_capacity(points.len());
    for &p in points.iter().filter(|p| p.is_finite()) {
        if out.last().is_none_or(|last| last.distance(p) > MERGE) {
            out.push(p);
        }
    }
    out
}

/// Distance along the line to each point.
fn cumulative(line: &[Vec3]) -> Vec<f32> {
    let mut acc = 0.0;
    let mut out = vec![0.0];
    for pair in line.windows(2) {
        acc += pair[0].distance(pair[1]);
        out.push(acc);
    }
    out
}

/// The line's direction at point `i`: from the previous point to the next, one
/// sided at an open end; a closed line wraps across its seam.
fn tangent_at(line: &[Vec3], i: usize, closed: bool) -> Vec3 {
    let last = line.len() - 1;
    let prev = match (i, closed) {
        (0, true) => line[last - 1],
        (0, false) => line[0],
        _ => line[i - 1],
    };
    let next = match (i == last, closed) {
        (true, true) => line[1],
        (true, false) => line[last],
        _ => line[i + 1],
    };
    (next - prev).normalize_or_zero()
}

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
