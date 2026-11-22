use crate::draw::batch::{OpaqueBatches, OpaqueOrder};
use crate::gpu::global_group::GlobalGroup;
use crate::gpu::groups;
use crate::gpu::ops::CountedEncoder as _;
use crate::gpu::pipelines::surface::SolidPass;
use crate::gpu::pipelines::PrimedPipelines;
use crate::ibl::skybox::SkyTargets;
use crate::motion;
use crate::passes::hiz::Culled;
use crate::passes::ssao::SsaoFrame;
use crate::postfx::effects::ssr;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};
use rusty_scene::ClearFlags;

#[derive(Clone, Copy)]
pub(crate) struct PassClear {
    pub color: Option<wgpu::Color>,
    pub flags: ClearFlags,
}

const BACKDROP: wgpu::Color = wgpu::Color {
    r: 0.06,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

impl PassClear {
    pub(crate) fn for_pass(is_first: bool, flags: ClearFlags) -> Self {
        let color = match flags {
            ClearFlags::Skybox | ClearFlags::SolidColor => Some(BACKDROP),
            ClearFlags::DepthOnly if is_first => Some(BACKDROP),
            ClearFlags::DepthOnly => None,
        };
        Self { color, flags }
    }
}

pub(crate) struct ScenePassFrame {
    pub clear: PassClear,
    pub ssao: Option<SsaoFrame>,
    pub ssil: Option<crate::postfx::effects::ssil::SsilFrame>,
    pub base: bool,
    pub screen_space: bool,
    pub culled: Option<Culled>,
    pub floored: bool,
}

impl ScenePassFrame {
    fn surface<'a>(&self, view: &'a RenderView) -> Option<[&'a wgpu::TextureView; 2]> {
        ssr::surface::views(&view.post_fx).filter(|_| self.base)
    }
}

fn scene_targets<'a>(
    view: &'a RenderView,
    frame: &ScenePassFrame,
) -> Vec<Option<wgpu::RenderPassColorAttachment<'a>>> {
    let clear = frame.clear;
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
    let velocity = velocity.map(|v| attachment(v, motion::CLEAR));
    let surface = frame.surface(view).into_iter().flatten();
    let surface = surface.map(|v| attachment(v, ssr::surface::CLEAR));
    std::iter::once(colour)
        .chain(velocity)
        .chain(surface)
        .collect()
}

impl Renderer {
    pub(crate) fn execute_scene_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &mut RenderView,
        frame: ScenePassFrame,
        solid_batches: &OpaqueBatches,
    ) {
        self.update_global_bind_group();
        let solids = (solid_batches, frame.culled.as_ref());
        self.record_screen_space(view, encoder, &frame, solids);
        self.record_scene_pass(view, encoder, &frame, solid_batches);
        let velocity = view.post_fx.motion.target_view().is_some();
        let primed = self.primes(view, &frame);
        debug_assert!(solid_batches.ranked_for(primed), "opaque order != priming");
        let counters = &mut self.frame_counters;
        counters.velocity_passes += u32::from(velocity);
        counters.primed_passes += u32::from(primed);
        counters.count_forward(solid_batches.iter(), primed);
        counters.ssr_surface_passes += u32::from(frame.surface(view).is_some());
    }

    fn record_scene_pass(
        &self,
        view: &RenderView,
        encoder: &mut wgpu::CommandEncoder,
        frame: &ScenePassFrame,
        solid_batches: &OpaqueBatches,
    ) {
        let clear = frame.clear;
        let velocity = view.post_fx.motion.target_view();
        let primed = self.prime_depth(view, frame);
        let targets = scene_targets(view, frame);
        let mut render_pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("Scene Render Pass"),
            color_attachments: &targets,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: match primed {
                        Some(_) => wgpu::LoadOp::Load,
                        None => wgpu::LoadOp::Clear(1.0),
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: self.gpu_timer.writes(GpuPass::Forward),
            occlusion_query_set: None,
        });
        let surface = frame.surface(view).is_some();
        let solid_pass = match velocity {
            Some(_) => SolidPass::Velocity(&self.velocity_pipeline, primed, surface),
            None => SolidPass::Opaque(view.forward_pipeline(&self.render_pipeline), primed),
        };
        render_pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
        render_pass.set_bind_group(groups::SCENE, self.scene_group(view, frame), &[]);
        let solids = (solid_batches, frame.culled.as_ref());
        self.draw_solids(&mut render_pass, solids, solid_pass);

        if clear.flags == ClearFlags::Skybox {
            let (sky, globals) = (&self.skybox_renderer, &self.global_bind_group);
            let targets = SkyTargets {
                velocity: velocity.is_some(),
                surface,
            };
            match &self.skybox_texture {
                Some(skybox_tex) => sky.draw(&mut render_pass, globals, skybox_tex, targets),
                None => sky.draw_gradient(&mut render_pass, globals, targets),
            }
        }
    }

    fn prime_depth(&self, view: &RenderView, frame: &ScenePassFrame) -> Option<&PrimedPipelines> {
        self.primes(view, frame).then_some(&self.primed_pipelines)
    }

    pub(crate) fn primes(&self, view: &RenderView, frame: &ScenePassFrame) -> bool {
        let prepass = frame.screen_space || frame.culled.is_some() || frame.floored;
        self.primes_by(view, prepass)
    }

    pub(crate) fn opaque_order(
        &self,
        view: &RenderView,
        cam: &rusty_scene::Camera,
        screen_space: bool,
    ) -> OpaqueOrder {
        OpaqueOrder::primed(self.primes_by(view, screen_space || self.hiz_culls(cam)))
    }

    fn primes_by(&self, view: &RenderView, prepass: bool) -> bool {
        let on = self.depth_priming && self.priming_recommended;
        // A view's own forward pipeline (the shader preview) has no primed twin.
        on && prepass && !view.has_forward_override()
    }

    pub(crate) fn update_global_bind_group(&mut self) {
        let switched = self.spare_global_group.switched;
        if !self.global_bind_group_dirty && !switched {
            return;
        }
        let only_sky = !self.global_bind_group_dirty;
        self.global_bind_group_dirty = false;
        let sky = &self.sky_reflection;
        let panorama = self.skybox_texture.as_deref();
        let cube = &self.reflection_probes;
        let build = || {
            GlobalGroup {
                camera: &self.camera_buffer.uniform,
                lighting: &self.lighting_buffer,
                skybox: sky.view(panorama),
                cube: &cube.view,
                clusters: &self.clusters,
                decals: &self.decals,
                lightmaps: self.lightmaps.binding(),
                fog_volume: self.fog_volume.view(),
                linear_clamp: &self.linear_clamp,
            }
            .create(&self.device, &self.camera_lighting_layout)
        };
        let slot = sky.procedural_slot(panorama);
        let group = &mut self.global_bind_group;
        self.spare_global_group
            .replace(group, (slot, only_sky), build);
    }
}
