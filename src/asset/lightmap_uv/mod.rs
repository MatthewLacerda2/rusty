//! src/asset/lightmap_uv/ — Generate Lightmap UVs (#831), Unity's import checkbox.
//!
//! Unwraps a mesh into a second, non-overlapping UV set the lightmap bake
//! (`scene::lighting::lightmap`) can rasterize, so a level exported from Blender
//! without a hand-made UV2 still gets a lightmap. Two well-known steps, no
//! dependency:
//!
//! * `chart` — split the triangles into **charts**: grow each one across shared
//!   edges while the edge is softer than the *hard angle* and the triangle still
//!   faces the chart's seed direction, then project it flat onto that direction.
//!   A triangle whose projection would overlap the chart starts a chart of its own,
//!   so no chart folds over itself.
//! * `pack` — shelf-pack the charts' rectangles, a *pack margin* apart, into the
//!   unit square.
//!
//! Charts are measured in world units and the margin is converted at
//! [`REFERENCE_TEXELS_PER_UNIT`], the bake's default resolution: since each mesh's
//! lightmap edge is its UV extent in world units times the bake's texels per unit,
//! `pack_margin` lands as that many texels between charts at the default resolution,
//! whatever the mesh's size.
//!
//! Deterministic: the same positions, indices and settings give the same UVs, bit
//! for bit (triangles are visited in index order, ties broken by index).

mod chart;
mod overlap;
mod pack;

use serde::{Deserialize, Serialize};

use super::mesh_data::{MeshVertex, SubMesh};

/// The texels per world unit the pack margin is measured at: the lightmap bake's
/// default `texelsPerUnit` (`BakeSettings::default`). A test holds the two equal.
pub const REFERENCE_TEXELS_PER_UNIT: f32 = 8.0;

/// A model's Generate Lightmap UVs setting, as its `.meta` sidecar stores it. Unity's
/// checkbox and two of its knobs; the rest (angle and area error) stay fixed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightmapUvSettings {
    /// Generate the lightmap UV at import, replacing an authored `TEXCOORD_1`. Off by
    /// default, as in Unity.
    pub generate: bool,
    /// Degrees: an edge whose faces meet at a sharper angle than this splits charts
    /// (Unity's *Hard Angle*, 0–180).
    pub hard_angle: f32,
    /// Texels between charts at the reference resolution (Unity's *Pack Margin*,
    /// 1–64), so bilinear filtering never reads a neighbouring chart.
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
    /// The settings with each knob clamped to Unity's range.
    pub fn clamped(self) -> Self {
        Self {
            generate: self.generate,
            hard_angle: self.hard_angle.clamp(0.0, 180.0),
            pack_margin: self.pack_margin.clamp(1.0, 64.0),
        }
    }
}

/// A generated unwrap. Charts split vertices along their seams, so the vertex list is
/// rebuilt: new vertex `i` copies source vertex `source[i]` and gets `uvs[i]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Unwrap {
    pub source: Vec<u32>,
    pub indices: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
}

/// Unwrap the triangle list `indices` over `positions` into non-overlapping charts
/// packed in `[0, 1]²`. Returns `None` when there is nothing to unwrap (no
/// triangles, or an index out of range).
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

/// One output vertex per (source vertex, chart) pair, in the order triangles first
/// use them, so a vertex on a seam is duplicated once per chart it borders.
fn split_vertices(indices: &[u32], chart_of: &[u32], corner_uvs: &[[f32; 2]]) -> Unwrap {
    // For each source vertex, the (chart, new index) pairs made so far.
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

/// Replace `sub`'s lightmap UV with a generated unwrap, splitting its vertices along
/// the chart seams. A skinned sub-mesh is left alone: the bake never lightmaps one.
pub fn apply(sub: &mut SubMesh, settings: &LightmapUvSettings) {
    if sub.skin.is_some() {
        return;
    }
    let with_uv = |v: &MeshVertex, lightmap_uv| MeshVertex { lightmap_uv, ..*v };
    if let Some((vertices, indices)) = unwrap_vertices(
        &sub.vertices,
        &sub.indices,
        |v| v.position,
        with_uv,
        settings,
    ) {
        (sub.vertices, sub.indices) = (vertices, indices);
    }
}

/// [`unwrap`] any vertex type: read each vertex's position with `position`, and
/// rebuild the split vertex list, each new vertex its source with the generated UV
/// written by `with_uv`. `None` when there is nothing to unwrap.
pub fn unwrap_vertices<V>(
    vertices: &[V],
    indices: &[u32],
    position: impl Fn(&V) -> [f32; 3],
    with_uv: impl Fn(&V, [f32; 2]) -> V,
    settings: &LightmapUvSettings,
) -> Option<(Vec<V>, Vec<u32>)> {
    let positions: Vec<[f32; 3]> = vertices.iter().map(position).collect();
    let u = unwrap(&positions, indices, settings)?;
    let out = u.source.iter().zip(&u.uvs);
    let out = out
        .map(|(&s, &uv)| with_uv(&vertices[s as usize], uv))
        .collect();
    Some((out, u.indices))
}

#[cfg(test)]
mod tests;
