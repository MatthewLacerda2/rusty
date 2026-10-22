//! The selection outline: an orange ring around exactly what the forward pass drew
//! for the selection (#1175). It is not a hull of its own: the forward pass marks
//! the solids it rings ([`outlined`]), records each one's draw as it batches them
//! (`FrameDraws::outline`), and [`Renderer::draw_outline`] draws that same mesh,
//! instance and per-draw uniform again through `outline.wgsl`. So the world matrix
//! and mirroring (#1142), the skinned pose and the LOD level (#1160), and any state
//! the forward draw gains later, are the forward pass's by construction. The shader
//! grows the ring by normal extrusion, so there is no pivot to grow about (#1165).

use crate::render::draw::batch::OutlineDraw;
use crate::render::gpu::draw_buffers::DrawBuffers;
use crate::render::lod::LodSelection;
use crate::render::Renderer;
use crate::scene::Scene;

impl Renderer {
    /// Draw the outline of each of `draws`: the forward pass's own buffers (plus the
    /// mesh's outline normals), group-1 slot and instance, through the outline pipeline (its clockwise twin for a
    /// mirrored solid). Group 0 and 3 are the scene pass's, already bound.
    pub(crate) fn draw_outline<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        draws: &'a [OutlineDraw],
    ) {
        for draw in draws {
            let Some(gpu_mesh) = self.gpu_meshes.get(&draw.mesh) else {
                continue;
            };
            let pipeline = if draw.mirrored {
                &self.mirrored_outline_pipeline
            } else {
                &self.outline_pipeline
            };
            render_pass.set_pipeline(pipeline.get());
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, gpu_mesh.outline_normals.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let offsets = DrawBuffers::offsets(draw.uniform_slot);
            render_pass.set_bind_group(1, self.draw_buffers.bind_group(), &offsets);
            // The outline samples no material; the default group satisfies the layout.
            render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
            render_pass.draw_indexed(0..draw.num_indices, 0, draw.instance..draw.instance + 1);
        }
    }
}

/// The entities the selection outline rings as `lod` shows them; empty with nothing
/// selected. The forward pass marks their draws (`DrawItem::outline`).
pub(crate) fn outlined(scene: &Scene, lod: &LodSelection) -> Vec<u32> {
    scene
        .selected_entity_id
        .map_or_else(Vec::new, |selected| outline_targets(scene, selected, lod))
}

/// The entities whose outlines ring `selected`. A LOD group's root, or a renderer the
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

#[cfg(test)]
mod draw_tests;
#[cfg(test)]
mod follow_tests;
#[cfg(test)]
mod tests;
