//! The reflection-probe gizmo's mirror ball (#1911): at each reflection probe's
//! capture point, a sphere [`BALL_DIAMETER`] across reflecting the probe's own cube,
//! Unity's reflection-probe preview sphere, so a glance shows what the probe puts on
//! glossy surfaces. A probe whose cube is not resident (unbaked, failed to load, or
//! past the resident count) shows a flat grey ball. It replaces the cross marker; the
//! parallax box stays a wireframe (`overlays::probes`).
//!
//! The cube is the forward pass's own: the resident probe cube array at group 0
//! (`render::ibl::reflection_probes`), sampled at the probe's layer, so the gizmo
//! loads nothing. The ball is drawn by `engine/shaders/gizmos/probe_ball.wgsl` (a
//! ray-cast sphere on a quad facing the view ray, writing its depth) through the
//! forward pipeline layout, so it binds the handles pass's groups as the lines do: its
//! placement rides an overlay draw slot, and its resident slot the instance index.

use glam::{Mat4, Vec3};

use super::{OverlayBindings, OverlayDraws};
use crate::core::application::{Gizmo, SceneGizmos};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::DEPTH_FORMAT;
use crate::render::Renderer;
use crate::scene::Scene;

/// The ball's diameter in world units, constant at any distance: Unity's
/// reflection-probe preview sphere is about half a metre across.
pub(crate) const BALL_DIAMETER: f32 = 0.5;

/// One ball for the handles pass: its slot of the overlays' draws, and its probe's
/// resident slot plus one (0: no resident cube), drawn as its instance index.
pub(crate) type BallResource = (u32, u32);

impl Renderer {
    /// One ball per reflection probe of `scene`, while `gizmos` has them on.
    pub(crate) fn precreate_probe_balls(
        &self,
        scene: &Scene,
        gizmos: &SceneGizmos,
        draws: &mut OverlayDraws,
    ) -> Vec<BallResource> {
        if !gizmos.on(Gizmo::ReflectionProbes) {
            return Vec::new();
        }
        let resident = |p| self.reflection_probes.resident(p);
        (scene.reflection_probes.probes.iter())
            .map(|p| {
                let slot = draws.push(ball_placement(p.position), [1.0; 4]);
                (slot, resident_instance(resident(p)))
            })
            .collect()
    }

    /// Draw `balls` into the handles pass, then put the line pipeline back. Groups 0,
    /// 2 and 3 are the ones the lines bound; each ball binds its slot of `bindings`.
    pub(super) fn draw_probe_balls<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        bindings: &'a OverlayBindings,
        balls: &[BallResource],
    ) {
        if balls.is_empty() {
            return;
        }
        pass.set_pipeline(self.probe_ball_pipeline.get());
        for &(slot, resident) in balls {
            bindings.bind(pass, slot);
            pass.draw(0..6, resident..resident + 1);
        }
        pass.set_pipeline(self.line_pipeline.get());
    }
}

/// The ball's overlay placement: centred on the probe, scaled to its radius (the
/// shader reads the centre from the translation and the radius from the first column).
pub(crate) fn ball_placement(position: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(BALL_DIAMETER * 0.5),
        glam::Quat::IDENTITY,
        position,
    )
}

/// The instance index a ball draws with: its probe's resident slot plus one, 0 when
/// its cube is not resident.
pub(crate) fn resident_instance(slot: Option<usize>) -> u32 {
    slot.map_or(0, |s| s as u32 + 1)
}

/// The ball's pipeline, built on its first draw: the forward `layout` (the handles
/// pass's groups), no vertex buffer, both faces, opaque into `output_format`, testing
/// and writing the view's depth.
pub(crate) fn ball_pipeline(
    device: &wgpu::Device,
    registry: &mut ShaderRegistry,
    layout: &wgpu::PipelineLayout,
    output_format: wgpu::TextureFormat,
) -> LazyPipeline {
    let shader = registry.load(device, "gizmos/probe_ball.wgsl", "Probe Ball Shader");
    let layout = layout.clone();
    LazyPipeline::new(device, move |device| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Probe Ball Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ball"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ball"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placement_carries_the_centre_and_radius() {
        let m = ball_placement(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(m.w_axis.truncate(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(m.x_axis.truncate().length(), BALL_DIAMETER * 0.5);
    }

    #[test]
    fn a_probe_with_no_resident_cube_draws_instance_zero() {
        assert_eq!(resident_instance(None), 0);
        assert_eq!(resident_instance(Some(0)), 1);
        assert_eq!(resident_instance(Some(3)), 4);
    }
}

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod ortho_gpu_tests;
