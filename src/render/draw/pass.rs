//! Shadow depth passes, dynamic global bind-group update, and the main scene
//! render pass. The per-camera stacking loop + post-FX live in `draw`.

use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::draw_buffers::DrawBuffers;
use crate::render::gpu::global_group::GlobalGroup;
use crate::render::gpu::pipelines::surface::SolidPass;
use crate::render::passes::ssao::SsaoFrame;
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};
use crate::scene::ClearFlags;

/// The framebuffer clear behavior for one camera in the stack (#93). Derived from the
/// camera's [`ClearFlags`] and whether it is the first (bottom) pass of the frame.
#[derive(Clone, Copy)]
pub(crate) struct PassClear {
    /// `Some(backdrop)` clears color to that RGBA; `None` loads the existing color so
    /// this camera composites on top of what is already drawn (`DepthOnly`).
    pub color: Option<wgpu::Color>,
    /// The camera's clear flags, kept so the pass only redraws the skybox for a
    /// `Skybox` camera (a `DepthOnly` overlay must not paint over the world).
    pub flags: ClearFlags,
}

/// The dark editor/world backdrop the base camera clears to.
const BACKDROP: wgpu::Color = wgpu::Color {
    r: 0.06,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

impl PassClear {
    /// Resolve the clear ops for a pass. The bottom pass always clears color (nothing
    /// is under it); `DepthOnly` preserves color so a viewmodel/UI camera layers on
    /// top of the world. Every camera clears depth, so stacked geometry never clips
    /// against the layer below — the FPS viewmodel fix.
    pub(crate) fn for_pass(is_first: bool, flags: ClearFlags) -> Self {
        let color = match flags {
            ClearFlags::Skybox | ClearFlags::SolidColor => Some(BACKDROP),
            // A DepthOnly bottom pass has nothing to composite over, so clear anyway.
            ClearFlags::DepthOnly if is_first => Some(BACKDROP),
            ClearFlags::DepthOnly => None,
        };
        Self { color, flags }
    }
}

/// Per-camera inputs threaded into the scene pass.
pub(crate) struct ScenePassFrame {
    pub clear: PassClear,
    /// This camera's SSAO (#436), recorded ahead of the scene pass; `None` when off.
    pub ssao: Option<SsaoFrame>,
}

/// The scene pass's colour attachments: the HDR target, and a motion-blurred view's
/// velocity target (#1435). Both clear where the camera clears colour (the backdrop,
/// "no motion") and keep the layer below where a `DepthOnly` camera composites over
/// it (#93), so the nearest camera's velocity wins as its colour does.
fn scene_targets(
    view: &RenderView,
    clear: PassClear,
) -> Vec<Option<wgpu::RenderPassColorAttachment<'_>>> {
    let attachment = |view, clear_to| {
        Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: clear
                    .color
                    .map_or(wgpu::LoadOp::Load, |_| wgpu::LoadOp::Clear(clear_to)),
                store: wgpu::StoreOp::Store,
            },
        })
    };
    let colour = attachment(
        &view.post_fx.scene_hdr.view,
        clear.color.unwrap_or(BACKDROP),
    );
    let velocity = view.post_fx.motion.target_view();
    let velocity = velocity.map(|v| attachment(v, wgpu::Color::TRANSPARENT));
    std::iter::once(colour).chain(velocity).collect()
}

impl Renderer {
    pub(crate) fn execute_scene_pass(
        &mut self,
        view: &mut RenderView,
        frame: ScenePassFrame,
        solid_batches: &[DrawBatch],
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Render Encoder"),
            });

        // The shadow maps were rendered once for the frame (`run_shadow_passes`).
        // Bind active skybox view & sampler into the global group for reflections.
        self.update_global_bind_group();
        // A. SSAO (#436): prepass, occlusion and blur, which the scene pass samples.
        if let Some(ssao) = &frame.ssao {
            self.record_ssao(view, &mut encoder, ssao, solid_batches);
        }
        // B. Main scene pass. The editor's gizmos draw after post-FX (`draw_handles`).
        self.record_scene_pass(view, &mut encoder, &frame, solid_batches);
        let velocity = view.post_fx.motion.target_view().is_some();
        self.frame_counters.velocity_passes += u32::from(velocity);

        // 6. Submit the scene pass (it filled the HDR target + depth). Particles +
        // post-FX run from the per-camera loop in `draw`.
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Record the main scene pass (solids, skybox) into `encoder`.
    fn record_scene_pass(
        &self,
        view: &RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &ScenePassFrame,
        solid_batches: &[DrawBatch],
    ) {
        let clear = frame.clear;
        let velocity = view.post_fx.motion.target_view();
        let targets = scene_targets(view, clear);
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("Scene Render Pass"),
            color_attachments: &targets,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view.depth_view,
                // Every camera clears depth so its geometry sorts independently of the
                // layer below — a viewmodel never clips through walls.
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: self.gpu_timer.writes(GpuPass::Forward),
            occlusion_query_set: None,
        });

        // Set global bindings
        let solid_pass = match velocity {
            Some(_) => SolidPass::Velocity(&self.velocity_pipeline),
            None => SolidPass::Opaque(view.forward_pipeline(&self.render_pipeline)),
        };
        render_pass.set_bind_group(0, &self.global_bind_group, &[]);
        render_pass.set_bind_group(3, self.scene_group3(view, frame.ssao.is_some()), &[]);
        // Solid entities (each batch binds its shader's pipeline).
        self.draw_batches(&mut render_pass, solid_batches, solid_pass);

        // Render Skybox last (optimization). Only a Skybox camera paints it — a
        // DepthOnly/SolidColor overlay must not overwrite the world below it. With a
        // panorama bound we draw it; with none, a Skybox camera falls back to the
        // procedural sky->horizon->ground gradient (#256) instead of the flat
        // backdrop, so an empty scene reads as a lit environment (Unity's default-
        // skybox fallback). Both paint only the far-plane pixels no geometry covered.
        if clear.flags == ClearFlags::Skybox {
            let (sky, globals) = (&self.skybox_renderer, &self.global_bind_group);
            let moving = velocity.is_some();
            match &self.skybox_texture {
                Some(skybox_tex) => sky.draw(&mut render_pass, globals, skybox_tex, moving),
                None => sky.draw_gradient(&mut render_pass, globals, moving),
            }
        }
    }

    /// Rebuild the group-0 bind group, but only when something it names changed — the
    /// skybox, the probe cube, or a light-cluster buffer that grew (#434). Its buffers
    /// are otherwise persistent (skips ~one bind-group creation per camera, #210).
    fn update_global_bind_group(&mut self) {
        if !self.global_bind_group_dirty {
            return;
        }
        self.global_bind_group_dirty = false;
        let sky = self.sky_panorama();
        // The resident reflection probes' cube array at binding 4 (#1206).
        let cube = &self.reflection_probes;
        self.global_bind_group = GlobalGroup {
            camera: &self.camera_buffer,
            lighting: &self.lighting_buffer,
            skybox: (&sky.view, &sky.sampler),
            cube: (&cube.view, &cube.sampler),
            clusters: &self.clusters,
            decals: &self.decals,
            lightmaps: self.lightmaps.binding(),
        }
        .create(&self.device, &self.camera_lighting_layout);
    }

    /// Record `batches` into `pass`: per batch, its pipeline when it differs from the
    /// last one (see [`SolidPass`]), the mesh buffers, the frame's group-1 bind group
    /// at the batch's uniform offset, the shared material group, and one instanced
    /// draw over its instance range (#470). Shared by the prepass, opaque and
    /// transparent passes.
    pub(crate) fn draw_batches<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        batches: &'a [DrawBatch],
        solid_pass: SolidPass<'a>,
    ) {
        let mut bound = None;
        for batch in batches {
            let Some(gpu_mesh) = self.gpu_meshes.get(&batch.key.mesh) else {
                continue;
            };
            if bound != Some(batch.key.pipeline) {
                render_pass.set_pipeline(self.surface_shaders.pick(solid_pass, batch.key.pipeline));
                bound = Some(batch.key.pipeline);
            }
            let offsets = DrawBuffers::offsets(batch.uniform_slot);
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, self.draw_buffers.bind_group(), &offsets);
            render_pass.set_bind_group(2, self.materials.group(batch.key.material), &[]);
            render_pass.draw_indexed(batch.key.indices.range(), 0, batch.instances.clone());
        }
    }
}
