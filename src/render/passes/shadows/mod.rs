//! The directional-light shadow pass: cascaded shadow maps fitted to the camera (#435).
//! Each cascade is one layer of a depth-array texture. Static casters are baked into a
//! cached static array per cascade, re-baked only when that cascade's light volume
//! moves (#355); each frame the static layers are copied into the active array and the
//! dynamic casters drawn over them. Casters are drawn instanced, one draw per mesh per
//! cascade (#470, `casters`), skinned ones in their animated pose (#599).

pub(crate) mod cascades;
mod casters;
mod frame;
mod setup;
mod uniform;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::lod::LodSelection;
use crate::scene::SceneId;
use cascades::{Cascade, MAX_CASCADES};
use casters::{CasterBuffer, CasterFrame};
use glam::Mat4;

pub(crate) use uniform::CascadeUniform;

/// Byte stride between the cascades' light-space matrices in the depth pass's uniform
/// buffer — the dynamic-offset alignment wgpu guarantees on every backend.
const LIGHT_SPACE_STRIDE: u64 = 256;

pub struct ShadowRenderer {
    static_texture: wgpu::Texture,
    active_texture: wgpu::Texture,
    /// One render-target view per cascade layer, static and active.
    static_layers: Vec<wgpu::TextureView>,
    active_layers: Vec<wgpu::TextureView>,
    /// The whole active array, as the forward shader samples it.
    pub active_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,

    pipeline: wgpu::RenderPipeline,
    light_space_buffer: wgpu::Buffer,
    /// This frame's cascades, fitted to the camera by [`Self::update_cascades`].
    pub(crate) cascades: Vec<Cascade>,

    /// Per cascade: which scene's static casters are baked into that layer, and under
    /// which light volume — `None` when it holds nothing usable (#355).
    ///
    /// Naming the scene keeps two scenes rendered in one frame (the editor viewport
    /// and the Inspector preview) from sampling each other's statics; naming the
    /// light volume re-bakes a cascade exactly when it moved (the camera crossed its
    /// snap grid, or the sun turned) and never otherwise.
    static_cache: [Option<(SceneId, Mat4)>; MAX_CASCADES],

    global_bind_group: wgpu::BindGroup,
    entity_layout: wgpu::BindGroupLayout,

    /// The static bake's and the dynamic pass's casters (#470) and joints (#599), every cascade
    /// packed into one array each — two, because both sweeps are recorded before one
    /// submit.
    static_casters: CasterBuffer,
    dynamic_casters: CasterBuffer,
    /// `(num_indices, instances)` of the caster draws since the renderer last cleared
    /// it — one frame's worth, read into the frame counters (#433).
    pub(crate) drawn: Vec<(u32, u32)>,
    /// Mirrors `Renderer::instancing`: off draws one caster per call (#470 tests).
    pub(crate) instancing: bool,
}

impl ShadowRenderer {
    /// Resolution of each cascade's layer. Four 1024² cascades are the same memory
    /// as the single 2048² map they replace (Unity's High tier splits one 2048 atlas
    /// the same way), with the first cascade covering a few metres instead of 60.
    pub const CASCADE_SIZE: u32 = 1024;

    pub fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let textures = Self::create_depth_textures(device);
        let sampler = Self::create_sampler(device);

        let light_space_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Light Space Buffer"),
            size: LIGHT_SPACE_STRIDE * MAX_CASCADES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (global_layout, global_bind_group, entity_layout) =
            Self::create_pass_layouts(device, &light_space_buffer);
        let pipeline = Self::create_pipeline(device, &global_layout, &entity_layout, registry);
        let static_casters = CasterBuffer::new(device, &entity_layout);
        let dynamic_casters = CasterBuffer::new(device, &entity_layout);

        Self {
            static_texture: textures.static_texture,
            active_texture: textures.active_texture,
            static_layers: textures.static_layers,
            active_layers: textures.active_layers,
            active_view: textures.active_view,
            sampler,
            pipeline,
            light_space_buffer,
            cascades: Vec::new(),
            static_cache: [None; MAX_CASCADES],
            global_bind_group,
            entity_layout,
            static_casters,
            dynamic_casters,
            drawn: Vec::new(),
            instancing: true,
        }
    }

    /// Whether some cascade's static bake is not `scene`'s: it holds another scene's
    /// statics, or nothing at all.
    pub fn needs_static_bake(&self, scene: SceneId) -> bool {
        let count = self.cascades.len().max(1);
        self.static_cache[..count]
            .iter()
            .any(|baked| baked.map(|(s, _)| s) != Some(scene))
    }

    /// Drop every static bake, so the next render re-bakes them. Called when the
    /// editor changes something that moves static geometry.
    pub fn invalidate_static_cache(&mut self) {
        self.static_cache = [None; MAX_CASCADES];
    }

    /// Adopt this frame's `cascades` and upload their light-space matrices.
    pub fn update_cascades(&mut self, queue: &wgpu::Queue, cascades: Vec<Cascade>) {
        for (i, cascade) in cascades.iter().enumerate() {
            queue.write_buffer(
                &self.light_space_buffer,
                i as u64 * LIGHT_SPACE_STRIDE,
                bytemuck::bytes_of(&cascade.light_space.to_cols_array()),
            );
        }
        self.cascades = cascades;
    }

    /// Record the frame's shadow sweeps: re-bake the static layers whose light volume
    /// moved, copy the static layers into the active array, and draw the dynamic
    /// casters over each cascade — skipping the LOD levels `lod` hides (#472).
    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) {
        let (scene, gpu_meshes) = (frame.scene, frame.gpu_meshes);
        let key = |c: &Cascade| Some((scene.id(), c.light_space));
        let stale: Vec<usize> = (0..self.cascades.len())
            .filter(|&i| self.static_cache[i] != key(&self.cascades[i]))
            .collect();
        if !stale.is_empty() {
            // The bake outlives the frame, so it cannot follow the camera's LOD
            // choice: it bakes every group at LOD0 (#472).
            let finest = LodSelection::finest(scene);
            let batches = self.prepare_casters(frame, &finest, true, &stale);
            for (&i, batches) in stale.iter().zip(&batches) {
                let view = &self.static_layers[i];
                let mut pass = depth_pass(encoder, "Shadow Static Pass", view, true);
                self.draw_casters(&mut pass, gpu_meshes, batches, true, i);
            }
            for i in stale {
                self.static_cache[i] = key(&self.cascades[i]);
            }
        }

        self.copy_static_layers(encoder);

        let all: Vec<usize> = (0..self.cascades.len()).collect();
        let batches = self.prepare_casters(frame, lod, false, &all);
        for (i, batches) in batches.iter().enumerate() {
            if batches.is_empty() {
                continue;
            }
            let view = &self.active_layers[i];
            let mut pass = depth_pass(encoder, "Shadow Dynamic Pass", view, false);
            self.draw_casters(&mut pass, gpu_meshes, batches, false, i);
        }
    }

    /// Copy the frame's cascade layers from the static bake into the active array.
    fn copy_static_layers(&self, encoder: &mut wgpu::CommandEncoder) {
        let layer = |texture| wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        encoder.copy_texture_to_texture(
            layer(&self.static_texture),
            layer(&self.active_texture),
            wgpu::Extent3d {
                width: Self::CASCADE_SIZE,
                height: Self::CASCADE_SIZE,
                depth_or_array_layers: self.cascades.len().max(1) as u32,
            },
        );
    }
}

/// A depth-only pass over one cascade layer: cleared for a bake, loaded for the
/// dynamic casters drawn over the copied statics.
fn depth_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    view: &'a wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    let load = if clear {
        wgpu::LoadOp::Clear(1.0)
    } else {
        wgpu::LoadOp::Load
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

#[cfg(test)]
mod cache_tests;
