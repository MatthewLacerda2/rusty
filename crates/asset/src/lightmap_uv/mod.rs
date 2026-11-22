//! Unity's Generate Lightmap UVs. Charts are in world units; the margin converts
//! at [`REFERENCE_TEXELS_PER_UNIT`]. Deterministic: ties break by index.

mod chart;
mod overlap;
mod pack;

pub use overlap::interiors_overlap;

use serde::{Deserialize, Serialize};

use super::mesh_data::{SubMesh, Vertex};

/// Equals the bake's default `texelsPerUnit` (a test holds the two equal).
pub const REFERENCE_TEXELS_PER_UNIT: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightmapUvSettings {
    /// Replaces an authored `TEXCOORD_1`.
    pub generate: bool,
    /// Degrees (Unity's *Hard Angle*, 0–180).
    pub hard_angle: f32,
    /// Unity's *Pack Margin*, 1–64, at the reference resolution.
    pub pack_margin: f32,
}

impl Default for LightmapUvSettings {
    fn default() -> Self {
        Self {
            generate: false,
            hard_angle: 88.0,
            pack_margin: 4.0,
        }
    }
}

impl LightmapUvSettings {
    /// A non-finite knob falls back to its default.
    pub fn clamped(self) -> Self {
        let d = Self::default();
        let knob = |v: f32, fallback: f32, lo, hi| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                fallback
            }
        };
        Self {
            generate: self.generate,
            hard_angle: knob(self.hard_angle, d.hard_angle, 0.0, 180.0),
            pack_margin: knob(self.pack_margin, d.pack_margin, 1.0, 64.0),
        }
    }
}

/// New vertex `i` copies source vertex `source[i]` and gets `uvs[i]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Unwrap {
    pub source: Vec<u32>,
    pub indices: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
}

/// `None` when no triangles or an index is out of range.
pub fn unwrap(
    positions: &[[f32; 3]],
    indices: &[u32],
    settings: &LightmapUvSettings,
) -> Option<Unwrap> {
    if indices.len() < 3 || indices.iter().any(|&i| i as usize >= positions.len()) {
        return None;
    }
    let settings = settings.clamped();
    let charts = chart::build(positions, indices, settings.hard_angle);
    let margin = settings.pack_margin / REFERENCE_TEXELS_PER_UNIT;
    let corner_uvs = pack::pack(&charts, margin);
    Some(split_vertices(
        indices,
        &chart::chart_of_triangle(&charts),
        &corner_uvs,
    ))
}

fn split_vertices(indices: &[u32], chart_of: &[u32], corner_uvs: &[[f32; 2]]) -> Unwrap {
    let mut made: Vec<Vec<(u32, u32)>> = vec![Vec::new(); max_index(indices) + 1];
    let mut out = Unwrap {
        source: Vec::new(),
        indices: Vec::with_capacity(indices.len()),
        uvs: Vec::new(),
    };
    for (corner, &v) in indices.iter().enumerate() {
        let chart = chart_of[corner / 3];
        let copies = &mut made[v as usize];
        let new = match copies.iter().find(|(c, _)| *c == chart) {
            Some(&(_, new)) => new,
            None => {
                let new = out.source.len() as u32;
                out.source.push(v);
                out.uvs.push(corner_uvs[corner]);
                copies.push((chart, new));
                new
            }
        };
        out.indices.push(new);
    }
    out
}

fn max_index(indices: &[u32]) -> usize {
    indices.iter().copied().max().unwrap_or(0) as usize
}

/// A skinned sub-mesh is left alone: the bake never lightmaps one.
pub fn apply(sub: &mut SubMesh, settings: &LightmapUvSettings) -> Option<Vec<u32>> {
    if sub.skin.is_some() {
        return None;
    }
    let with_uv = |v: &Vertex, lightmap_uv| Vertex { lightmap_uv, ..*v };
    let (vertices, indices, source) = split(
        &sub.vertices,
        &sub.indices,
        |v| v.position,
        with_uv,
        settings,
    )?;
    (sub.vertices, sub.indices) = (vertices.into(), indices.into());
    Some(source)
}

pub fn unwrap_vertices<V>(
    vertices: &[V],
    indices: &[u32],
    position: impl Fn(&V) -> [f32; 3],
    with_uv: impl Fn(&V, [f32; 2]) -> V,
    settings: &LightmapUvSettings,
) -> Option<(Vec<V>, Vec<u32>)> {
    split(vertices, indices, position, with_uv, settings).map(|(v, i, _)| (v, i))
}

fn split<V>(
    vertices: &[V],
    indices: &[u32],
    position: impl Fn(&V) -> [f32; 3],
    with_uv: impl Fn(&V, [f32; 2]) -> V,
    settings: &LightmapUvSettings,
) -> Option<(Vec<V>, Vec<u32>, Vec<u32>)> {
    let positions: Vec<[f32; 3]> = vertices.iter().map(position).collect();
    let u = unwrap(&positions, indices, settings)?;
    let out = u.source.iter().zip(&u.uvs);
    let out = out
        .map(|(&s, &uv)| with_uv(&vertices[s as usize], uv))
        .collect();
    Some((out, u.indices, u.source))
}

#[cfg(test)]
mod tests;
