//! Undo records a mesh without geometry; a restore takes it from the live mesh of the
//! same source.

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
            own_geometry: self.own_geometry,
            clusters: self.clusters.clone(),
            ..self.without_geometry()
        }
    }
}

impl MeshComponent {
    /// Names every field: a new one goes here, and in `Clone` if heavy.
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
            own_geometry: None,
            clusters: None,
        }
    }

    /// Copies a shared buffer; a new revision and identity draw the edit on this mesh only.
    pub fn vertices_mut(&mut self) -> &mut Vec<Vertex> {
        self.renew(!self.vertices.is_unique());
        self.vertices.edit()
    }

    pub fn indices_mut(&mut self) -> &mut Vec<u32> {
        self.renew(!self.indices.is_unique());
        self.indices.edit()
    }

    /// A `shared` buffer: another mesh (a clone) may hold this identity.
    fn renew(&mut self, shared: bool) {
        self.revision = MeshRevision::next();
        if shared || self.own_geometry.is_none() {
            self.own_geometry = Some(self.revision);
        }
    }

    /// Whether a restore keeps the live geometry, in-place edits included (no Undo step).
    pub fn same_source(&self, other: &Self) -> bool {
        self.primitive_type == other.primitive_type
            && self.asset_ref == other.asset_ref
            && self.subdivisions == other.subdivisions
    }

    /// The renderer has this geometry: nothing uploads again.
    pub fn adopt_geometry(&mut self, from: &mut Self) {
        self.vertices = std::mem::take(&mut from.vertices);
        self.indices = std::mem::take(&mut from.indices);
        self.ranges = std::mem::take(&mut from.ranges);
        self.bind_palette = std::mem::take(&mut from.bind_palette);
        self.skin = from.skin.take();
        self.clips = std::mem::take(&mut from.clips);
        self.pose_palette = std::mem::take(&mut from.pose_palette);
        self.revision = from.revision;
        self.own_geometry = from.own_geometry.take();
        self.clusters = from.clusters.take();
    }
}
