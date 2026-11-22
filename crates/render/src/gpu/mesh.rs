use rusty_components::mesh::Vertex;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MeshId(pub String);

impl MeshId {
    pub fn from_mesh(mesh: &rusty_scene::MeshComponent) -> Self {
        let (kind, asset) = (&mesh.primitive_type, mesh.asset_ref.as_deref());
        let cuts = Some(mesh.subdivisions).filter(|&n| n != 1);
        let cuts = cuts.map(|n| format!("|{n}")).unwrap_or_default();
        let own = mesh.own_geometry.map(|r| format!("|own{r:?}"));
        let own = own.unwrap_or_default();
        MeshId(format!("{kind}|{}{cuts}{own}", asset.unwrap_or("")))
    }
}

const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x3,
    2 => Float32x2,
    3 => Uint32x4,
    4 => Float32x4,
    5 => Float32x4,
    6 => Float32x2,
];

pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

impl crate::Renderer {
    /// Upload `mesh`'s geometry and clusters under `id` at `revision`: what the
    /// extract (`upload_meshes_of`) and a load's warm-up (`render::warm`) both do
    /// with each slot [`stale_uploads`](crate::debug_meshes::stale_uploads)
    /// names, so the two can never upload differently.
    pub(crate) fn upload_geometry(
        &mut self,
        id: MeshId,
        revision: rusty_components::MeshRevision,
        mesh: &rusty_scene::MeshComponent,
    ) {
        self.update_gpu_mesh(id.clone(), revision, &mesh.vertices, &mesh.indices);
        self.attach_clusters(&id, (mesh.clusters(), &mesh.indices));
    }
}

impl crate::GpuMesh {
    pub(crate) fn runs(
        &self,
        mesh: &rusty_scene::MeshComponent,
        material: Option<&rusty_components::MaterialComponent>,
    ) -> Vec<(usize, std::ops::Range<u32>)> {
        let runs = rusty_components::mesh::clip_runs(mesh.index_ranges(), self.num_indices);
        rusty_components::mesh::material_runs(runs, material.map(|m| m.materials.len()))
    }
}
