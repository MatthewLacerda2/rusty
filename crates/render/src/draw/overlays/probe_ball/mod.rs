use glam::{Mat4, Vec3};

use super::{OverlayBindings, OverlayDraws};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::DEPTH_FORMAT;
use crate::Renderer;
use rusty_core::application::{Gizmo, SceneGizmos};
use rusty_scene::Scene;

/// Metres in the world, the same at any distance: Unity's reflection-probe preview sphere
/// is about half a metre.
pub(crate) const BALL_DIAMETER: f32 = 0.5;

pub(crate) type BallResource = (u32, u32);

impl Renderer {
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

/// `probe_ball.wgsl` reads the centre from the translation and the radius from column 0's length.
pub(crate) fn ball_placement(position: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(BALL_DIAMETER * 0.5),
        glam::Quat::IDENTITY,
        position,
    )
}

pub(crate) fn resident_instance(slot: Option<usize>) -> u32 {
    slot.map_or(0, |s| s as u32 + 1)
}

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
                depth_compare: Some(crate::gpu::depth::NEARER_OR_EQUAL),
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
