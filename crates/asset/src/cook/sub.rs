//! crates/asset/src/cook/sub.rs — one sub-mesh's cooked values, and how they are
//! laid back onto a fresh parse (#2332).
//!
//! What a cook computes per sub-mesh is kept: every final vertex's tangent and
//! lightmap UV, the vertex of the parse each final vertex copies (only when the
//! unwrap split vertices; otherwise vertex `i` is parsed vertex `i`), the indices
//! in cluster order and the cluster table. Everything else a vertex holds
//! (position, normal, UV, skin binding) is read from the parse itself, after the
//! Scale Factor, so a cooked import is the same value a fresh one is.

use std::sync::Arc;

use crate::clusters::Clusters;
use crate::mesh_data::{MeshVertex, SubMesh};

#[derive(Clone, Debug, PartialEq)]
pub struct Cooked {
    pub id: String,
    /// The parsed vertex each final vertex copies; `None` for one to one.
    pub source: Option<Vec<u32>>,
    /// Each final vertex's position, scaled: what [`Cooked::apply`] checks the
    /// parse against, so values cooked from another model are never laid on it.
    pub positions: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
    pub lightmap_uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub clusters: Option<Arc<Clusters>>,
}

impl Cooked {
    /// The cooked values of `sub`, fully cooked, whose unwrap made `source`.
    pub fn of(sub: &SubMesh, source: Option<Vec<u32>>) -> Self {
        Self {
            id: sub.id.clone(),
            source,
            positions: sub.vertices.iter().map(|v| v.position).collect(),
            tangents: sub.vertices.iter().map(|v| v.tangent).collect(),
            lightmap_uvs: sub.vertices.iter().map(|v| v.lightmap_uv).collect(),
            indices: sub.indices.clone(),
            clusters: sub.clusters.clone(),
        }
    }

    /// Lay these values onto `sub`, freshly parsed and scaled, or say why they do
    /// not fit it (another id, a count that differs, an index past its vertices):
    /// `sub` is then left as it was.
    pub fn apply(self, sub: &mut SubMesh) -> Result<(), String> {
        if self.id != sub.id {
            return Err(format!(
                "sub-mesh {:?} where {:?} was parsed",
                self.id, sub.id
            ));
        }
        let parsed = sub.vertices.len();
        let count = self.source.as_ref().map_or(parsed, Vec::len);
        let sources_fit = (self.source.iter().flatten()).all(|&s| (s as usize) < parsed);
        let fits = sources_fit
            && self.positions.len() == count
            && self.tangents.len() == count
            && self.lightmap_uvs.len() == count
            && self.indices.len() == sub.indices.len()
            && self.indices.iter().all(|&i| (i as usize) < count)
            && (self.clusters.iter().flat_map(|c| &c.list)).all(|c| {
                c.indices.start <= c.indices.end && c.indices.end as usize <= self.indices.len()
            });
        if !fits {
            return Err(format!(
                "{:?}'s cooked values do not fit its parse",
                self.id
            ));
        }
        let at = |i: usize| match &self.source {
            Some(s) => s[i] as usize,
            None => i,
        };
        let bits = |p: [f32; 3]| p.map(f32::to_bits);
        let same = |i: usize| bits(self.positions[i]) == bits(sub.vertices[at(i)].position);
        if let Some(i) = (0..count).find(|&i| !same(i)) {
            return Err(format!(
                "{:?}'s vertex {i} was cooked at another position",
                self.id
            ));
        }
        let vertex = |i: usize| MeshVertex {
            tangent: self.tangents[i],
            lightmap_uv: self.lightmap_uvs[i],
            ..sub.vertices[at(i)]
        };
        sub.vertices = (0..count).map(vertex).collect();
        sub.indices = self.indices;
        sub.clusters = self.clusters;
        Ok(())
    }
}
