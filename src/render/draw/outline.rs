//! The selection outline: an orange hull drawn behind exactly what the forward pass
//! drew for the selection. Each hull sits at its entity's world matrix (#1142), grown
//! slightly about its pivot, in the pose its skin shows and at the LOD level the
//! camera shows (#1160). Both come from the forward pass's own sources, never a
//! second computation: the palette from `MeshComponent::active_palette` through
//! `push_palette`, the level from the camera's `LodSelection`.

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::render::gpu::draw_buffers::{joint_array_bytes, push_palette};
use crate::render::gpu::pipelines::winding::is_mirrored;
use crate::render::lod::LodSelection;
use crate::render::{EntityUniform, MeshId, Renderer};
use crate::scene::Scene;

/// One outline hull draw: the mesh it hulls, its index count, the bind group over
/// its uniform and bone palette, and whether its world matrix mirrors it (it then
/// draws through the clockwise twin, #1142).
pub(crate) struct OutlineHull {
    pub mesh: MeshId,
    pub num_indices: u32,
    pub bind_group: wgpu::BindGroup,
    pub mirrored: bool,
}

impl Renderer {
    /// The hulls ringing the selection as `lod` shows it; empty with nothing selected.
    pub(crate) fn precreate_outline(&self, scene: &Scene, lod: &LodSelection) -> Vec<OutlineHull> {
        let Some(selected) = scene.selected_entity_id else {
            return Vec::new();
        };
        outline_targets(scene, selected, lod)
            .into_iter()
            .filter_map(|id| self.outline_hull_draw(scene, id))
            .collect()
    }

    /// `id`'s hull: its world matrix grown about its pivot, skinned by the palette
    /// the forward pass binds for it. `None` without a resident mesh.
    fn outline_hull_draw(&self, scene: &Scene, id: u32) -> Option<OutlineHull> {
        let mesh = scene.world.mesh(id)?;
        let mesh_id = MeshId::from_mesh(&mesh);
        let num_indices = self.gpu_meshes.get(&mesh_id)?.num_indices;
        let world = scene.world_matrix(id);
        let mut joints = Vec::new();
        let mut uniform = outline_uniform(outline_hull(world));
        uniform.bone_base = push_palette(&mut joints, mesh.active_palette());
        let init = |label, contents: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
        };
        let uniform_buf = init(
            "Outline Entity Uniform",
            bytemuck::bytes_of(&uniform),
            wgpu::BufferUsages::UNIFORM,
        );
        // A non-skinned hull binds the shared identity joint; a skinned one its pose.
        let bones = (!joints.is_empty()).then(|| {
            init(
                "Outline Bones",
                &joint_array_bytes(&joints),
                wgpu::BufferUsages::STORAGE,
            )
        });
        let bones = bones.as_ref().unwrap_or(self.shared_bones_buffer());
        Some(OutlineHull {
            mesh: mesh_id,
            num_indices,
            bind_group: self.entity_bind_group("Outline Bind Group", &uniform_buf, bones),
            mirrored: is_mirrored(world),
        })
    }
}

/// The entities whose hulls outline `selected`. A LOD group's root, or a renderer the
/// current level hides, stands for the level `lod` shows (#1160): selecting LOD0 while
/// LOD2 draws rings LOD2, and a culled group rings nothing. A group's own mesh rings
/// with its level. A shown renderer, like anything else, is outlined alone, as Unity
/// does: selecting Hat in a `[Body, Hat]` level rings Hat. Only active, shown meshes
/// ring.
pub(super) fn outline_targets(scene: &Scene, selected: u32, lod: &LodSelection) -> Vec<u32> {
    if !scene.world.is_active(selected) {
        return Vec::new();
    }
    let mut ids = vec![selected];
    let stands_for_level = |&g: &u32| g == selected || lod.hides(selected);
    if let Some(group_id) = owning_group(scene, selected).filter(stands_for_level) {
        let group = scene
            .world
            .lod_group(group_id)
            .expect("owning_group found it");
        ids = std::iter::once(group_id).chain(group.renderers()).collect();
    }
    let mut seen = Vec::new();
    ids.retain(|&id| {
        let fresh = !seen.contains(&id);
        seen.push(id);
        fresh && scene.world.has_mesh(id) && scene.world.is_active(id) && !lod.hides(id)
    });
    ids
}

/// The active LOD group `id` is, or lists as a renderer. An inactive group hides
/// nothing (`LodSelection`), so it owns nothing here either.
fn owning_group(scene: &Scene, id: u32) -> Option<u32> {
    let world = &scene.world;
    if world.has_lod_group(id) {
        return Some(id);
    }
    world.ids_with_lod_group().into_iter().find(|&g| {
        world.is_active(g)
            && world
                .lod_group(g)
                .is_some_and(|l| l.renderers().any(|r| r == id))
    })
}

/// Where the outline hull sits: the selection's own world matrix (#1142), so a child
/// follows its parents, scaled up slightly about its pivot.
pub(super) fn outline_hull(world: Mat4) -> Mat4 {
    world * Mat4::from_scale(Vec3::splat(1.05))
}

/// The fixed outline-hull uniform (orange tint, unlit) at `model`.
fn outline_uniform(model: Mat4) -> EntityUniform {
    EntityUniform {
        model_matrix: model.to_cols_array(),
        color_tint: [1.0, 0.5, 0.0, 1.0], // Vibrant glowing orange outline
        use_texture: 0,
        is_lit: 0,
        metallic: 0.0,
        roughness: 0.5,
        use_metallic_map: 0,
        use_roughness_map: 0,
        use_normal_map: 0,
        use_emissive_map: 0,
        emissive: [0.0; 4],
        use_cutout: 0,
        alpha_cutoff: 0.0,
        bone_base: 0,
        receive_decals: 0,
    }
}
