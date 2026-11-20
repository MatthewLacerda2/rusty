//! crates/components/src/mesh/geometry.rs — a mesh without its geometry, and back (#1631).
//!
//! The editor's Undo records an entity's state before every edit, and a model's
//! vertices are megabytes the edit almost never touched. So a record keeps the
//! mesh's saved fields and drops the heavy rehydrated ones; restoring takes the
//! geometry back from the live mesh when it still comes from the same source.
//!
//! The vertex and index buffers are shared (`rusty_asset::mesh_data::Shared`,
//! #2351) with the import cache and every other instance, so a [`Clone`] of a mesh
//! points at the same geometry. An edit in place goes through
//! [`MeshComponent::vertices_mut`] or [`MeshComponent::indices_mut`], which copy
//! the buffer first when anyone else holds it (counted by `mesh::copies`) and
//! renew the revision; replacing a buffer whole (`mesh.vertices = vec.into()`)
//! copies nothing.

use super::{MeshComponent, MeshRevision, Vertex};

/// A full copy: [`MeshComponent::without_geometry`]'s fields (every one named
/// there) with the heavy ones too, the vertices and indices shared, not copied.
impl Clone for MeshComponent {
    fn clone(&self) -> Self {
        Self {
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            ranges: self.ranges.clone(),
            bind_palette: self.bind_palette.clone(),
            skin: self.skin.clone(),
            clips: self.clips.clone(),
            pose_palette: self.pose_palette.clone(),
            clusters: self.clusters.clone(),
            ..self.without_geometry()
        }
    }
}

impl MeshComponent {
    /// This mesh with every saved field and every light runtime one copied, and its
    /// heavy rehydrated ones (vertices, indices, runs, palettes, skin, clips, clusters) left
    /// empty. Every field is named, so a new one must be placed here.
    pub fn without_geometry(&self) -> Self {
        Self {
            primitive_type: self.primitive_type.clone(),
            asset_ref: self.asset_ref.clone(),
            subdivisions: self.subdivisions,
            cast_shadows: self.cast_shadows,
            scale_in_lightmap: self.scale_in_lightmap,
            vertices: Default::default(),
            indices: Default::default(),
            ranges: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: self.skeleton.clone(),
            max_draw_distance: self.max_draw_distance,
            min_screen_size: self.min_screen_size,
            revision: self.revision,
            clusters: None,
        }
    }

    /// The vertices, to edit in place: this mesh's own copy when another mesh or
    /// the import cache shares them, at a new revision so every renderer re-uploads
    /// them. A primitive's edit needs a new `MeshId` too (see `revision`).
    pub fn vertices_mut(&mut self) -> &mut Vec<Vertex> {
        self.revision = MeshRevision::next();
        self.vertices.edit()
    }

    /// The indices, to edit in place, as [`Self::vertices_mut`] gives the vertices.
    pub fn indices_mut(&mut self) -> &mut Vec<u32> {
        self.revision = MeshRevision::next();
        self.indices.edit()
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
        self.clusters = from.clusters.take();
    }
}
