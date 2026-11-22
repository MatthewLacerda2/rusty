use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::{OverlayBindings, OverlayDraws};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::DEPTH_FORMAT;
use crate::Renderer;
use rusty_core::application::{Gizmo, SceneGizmos};

pub(crate) const NAVMESH_TINT: [f32; 4] = [0.2, 0.8, 1.0, 0.4];

const VERTEX_BYTES: u64 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavMeshSource {
    pub instance: usize,
    pub width: i32,
    pub height: i32,
    pub grid: [u32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavMeshRevision {
    pub source: NavMeshSource,
    pub generation: u64,
}

pub struct NavMeshPatch<I> {
    pub revision: NavMeshRevision,
    pub reset: Option<usize>,
    pub tiles: I,
}

pub(crate) struct NavMeshGizmo {
    pipeline: LazyPipeline,
    revision: Option<NavMeshRevision>,
    tiles: Vec<Option<(wgpu::Buffer, u32)>>,
    warned: bool,
}

impl NavMeshGizmo {
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        layout: &wgpu::PipelineLayout,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = registry.load(device, "gizmos/navmesh.wgsl", "NavMesh Gizmo Shader");
        let layout = layout.clone();
        let pipeline = LazyPipeline::new(device, move |device| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("NavMesh Gizmo Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_navmesh"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: VERTEX_BYTES,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_navmesh"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: output_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        Self {
            pipeline,
            revision: None,
            tiles: Vec::new(),
            warned: false,
        }
    }

    pub(crate) fn pipeline(&self) -> &LazyPipeline {
        &self.pipeline
    }
}

pub(crate) fn vertices_that_fit(len: usize, max_bytes: u64) -> usize {
    let fit = (max_bytes / VERTEX_BYTES) as usize;
    len.min(fit - fit % 6)
}

impl Renderer {
    pub fn navmesh_revision(&self) -> Option<NavMeshRevision> {
        self.navmesh.revision
    }

    pub fn set_navmesh<I>(&mut self, patch: NavMeshPatch<I>) -> u64
    where
        I: IntoIterator<Item = (usize, Vec<Vec3>)>,
    {
        let gizmo = &mut self.navmesh;
        if let Some(count) = patch.reset {
            gizmo.tiles = (0..count).map(|_| None).collect();
        }
        gizmo.revision = Some(patch.revision);
        let max_bytes = self.device.limits().max_buffer_size;
        let mut bytes = 0;
        for (index, triangles) in patch.tiles {
            let Some(slot) = gizmo.tiles.get_mut(index) else {
                continue;
            };
            let count = vertices_that_fit(triangles.len(), max_bytes);
            if count < triangles.len() && !gizmo.warned {
                gizmo.warned = true;
                log::warn!("[Gizmos] a navmesh tile is too large to draw whole: part is left out");
            }
            bytes += count as u64 * VERTEX_BYTES;
            *slot = (count > 0).then(|| {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("NavMesh Gizmo Tile"),
                        contents: bytemuck::cast_slice(&triangles[..count]),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                (buffer, count as u32)
            });
        }
        bytes
    }

    pub(crate) fn precreate_navmesh(
        &self,
        gizmos: &SceneGizmos,
        draws: &mut OverlayDraws,
    ) -> Option<u32> {
        if !gizmos.on(Gizmo::NavMesh) || self.navmesh.tiles.iter().all(Option::is_none) {
            return None;
        }
        Some(draws.push(Mat4::IDENTITY, NAVMESH_TINT))
    }

    pub(super) fn draw_navmesh<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        bindings: &'a OverlayBindings,
        slot: Option<u32>,
    ) {
        let Some(slot) = slot else {
            return;
        };
        pass.set_pipeline(self.navmesh.pipeline.get());
        bindings.bind(pass, slot);
        for (buffer, count) in self.navmesh.tiles.iter().flatten() {
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
        pass.set_pipeline(self.line_pipeline.get());
    }
}

#[cfg(test)]
#[path = "navmesh_tests.rs"]
mod tests;
