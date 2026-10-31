//! src/components/mesh/geometry.rs — a mesh without its geometry, and back (#1631).
//!
//! The editor's Undo records an entity's state before every edit, and a model's
//! vertices are megabytes the edit almost never touched. So a record keeps the
//! mesh's saved fields and drops the heavy rehydrated ones; restoring takes the
//! geometry back from the live mesh when it still comes from the same source.

use super::MeshComponent;

impl MeshComponent {
    /// This mesh with every saved field and every light runtime one copied, and its
    /// heavy rehydrated ones (vertices, indices, runs, palettes, skin, clips) left
    /// empty. Every field is named, so a new one must be placed here.
    pub fn without_geometry(&self) -> Self {
        Self {
            primitive_type: self.primitive_type.clone(),
            asset_ref: self.asset_ref.clone(),
            subdivisions: self.subdivisions,
            cast_shadows: self.cast_shadows,
            vertices: Vec::new(),
            indices: Vec::new(),
            ranges: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: self.skeleton.clone(),
            revision: self.revision,
        }
    }

    /// Whether `other`'s geometry is this mesh's: the same primitive or asset, cut
    /// the same way. What decides that a restore can keep the live geometry.
    pub fn same_source(&self, other: &Self) -> bool {
        self.primitive_type == other.primitive_type
            && self.asset_ref == other.asset_ref
            && self.subdivisions == other.subdivisions
    }

    /// Move `from`'s heavy rehydrated fields (the ones [`Self::without_geometry`]
    /// empties) and its revision onto this mesh: the renderer has that geometry
    /// already, so nothing is uploaded again.
    pub fn adopt_geometry(&mut self, from: &mut Self) {
        self.vertices = std::mem::take(&mut from.vertices);
        self.indices = std::mem::take(&mut from.indices);
        self.ranges = std::mem::take(&mut from.ranges);
        self.bind_palette = std::mem::take(&mut from.bind_palette);
        self.skin = from.skin.take();
        self.clips = std::mem::take(&mut from.clips);
        self.pose_palette = std::mem::take(&mut from.pose_palette);
        self.revision = from.revision;
    }
}
