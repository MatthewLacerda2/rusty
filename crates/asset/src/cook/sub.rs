//! One sub-mesh's cooked values, and how they are laid back onto a fresh parse.
//!
//! Kept per sub-mesh: tangents, lightmap UVs, the cluster-ordered indices and cluster
//! table, and (only when the unwrap split vertices) the parsed vertex each copies.
//! Everything else is read from the parse. A cooked sub-mesh also keeps the [`digest`]
//! of its parse and is laid only on a parse with the same one: the stamp names files
//! and settings, not the parser.

use std::sync::Arc;

use crate::clusters::Clusters;
use crate::mesh_data::{SubMesh, Vertex};

#[derive(Clone, Debug, PartialEq)]
pub struct Cooked {
    pub id: String,
    pub parse: u64,
    pub source: Option<Vec<u32>>,
    pub positions: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
    pub lightmap_uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub clusters: Option<Arc<Clusters>>,
}

impl Cooked {
    pub fn of(sub: &SubMesh, parse: u64, source: Option<Vec<u32>>) -> Self {
        Self {
            id: sub.id.clone(),
            parse,
            source,
            positions: sub.vertices.iter().map(|v| v.position).collect(),
            tangents: sub.vertices.iter().map(|v| v.tangent).collect(),
            lightmap_uvs: sub.vertices.iter().map(|v| v.lightmap_uv).collect(),
            indices: sub.indices.to_vec(),
            clusters: sub.clusters.clone(),
        }
    }

    pub fn apply(self, sub: &mut SubMesh) -> Result<(), String> {
        if self.id != sub.id {
            return Err(format!(
                "sub-mesh {:?} where {:?} was parsed",
                self.id, sub.id
            ));
        }
        if self.parse != digest(sub) {
            return Err(format!("{:?} was cooked from another parse", self.id));
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
        let vertex = |i: usize| Vertex {
            tangent: self.tangents[i],
            lightmap_uv: self.lightmap_uvs[i],
            ..sub.vertices[at(i)]
        };
        sub.vertices = (0..count).map(vertex).collect();
        sub.indices = self.indices.into();
        sub.clusters = self.clusters;
        Ok(())
    }
}

pub fn digest(sub: &SubMesh) -> u64 {
    let mut hash = rusty_core::hash::Content::default();
    let mut buf: Vec<u8> = Vec::with_capacity(VERTEX * BATCH);
    for batch in sub.vertices.chunks(BATCH) {
        buf.clear();
        for v in batch {
            let floats = (v.position.iter().chain(&v.normal).chain(&v.tex_coords))
                .chain(&v.lightmap_uv)
                .chain(&v.joint_weights);
            floats.for_each(|f| buf.extend_from_slice(&f.to_bits().to_le_bytes()));
            v.joint_indices
                .iter()
                .for_each(|j| buf.extend_from_slice(&j.to_le_bytes()));
        }
        hash.write(&buf);
    }
    hash.write(&[u8::from(sub.skin.is_some())]);
    for n in [sub.vertices.len(), sub.indices.len(), sub.ranges.len()] {
        hash.write(&(n as u64).to_le_bytes());
    }
    for chunk in sub.indices.chunks(BATCH * 4) {
        buf.clear();
        chunk
            .iter()
            .for_each(|i| buf.extend_from_slice(&i.to_le_bytes()));
        hash.write(&buf);
    }
    for r in &sub.ranges {
        let material = r.material.map_or(u64::MAX, |m| m as u64);
        hash.write(&r.indices.start.to_le_bytes());
        hash.write(&r.indices.end.to_le_bytes());
        hash.write(&material.to_le_bytes());
    }
    hash.finish()
}

/// The bytes [`digest`] reads of one vertex: 14 floats and 4 joint indices.
const VERTEX: usize = 18 * 4;
const BATCH: usize = 1024;
