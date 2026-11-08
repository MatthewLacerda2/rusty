//! The NavMesh gizmo (#1910): the baked navmesh as translucent cyan over the floor
//! in the Scene view, under the `NavMesh` gizmo flag, Unity's navmesh overlay.
//!
//! **Render never sees the graph.** `render` may not import `navigation`, so the
//! shell turns the graph into world-space triangles (one quad per walkable span, on
//! the span's floor plane) and hands them over with a revision through
//! [`Renderer::set_navmesh`]. Render only uploads them.
//!
//! **Built when the graph changes, never per frame.** The other overlays rebuild
//! their buffers every frame; a Dust2-sized navmesh is hundreds of thousands of
//! quads, so the vertex buffer is kept ([`Kept`]) and rebuilt only when the revision
//! the shell names moves (a bake, an edit-mode rebake, a load). A frame with the
//! same revision builds no triangle and uploads nothing.
//!
//! **Drawn translucent.** Its own pipeline over `engine/shaders/gizmos/navmesh.wgsl`:
//! a bare 12-byte position per vertex, alpha-blended, depth-tested against the
//! scene without writing it, after the opaque probe balls and before the lines, so
//! the grid and the collider wires stay crisp over it.

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::{OverlayBindings, OverlayDraws};
use crate::core::application::{Gizmo, SceneGizmos};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::DEPTH_FORMAT;
use crate::render::Renderer;

/// Unity's navmesh blue-cyan, translucent so the floor reads through it.
pub(crate) const NAVMESH_TINT: [f32; 4] = [0.2, 0.8, 1.0, 0.4];

/// One vertex's bytes: a world position.
const VERTEX_BYTES: u64 = 12;

/// A value built from a source named by a revision, rebuilt only when it moves.
pub(crate) struct Kept<B> {
    revision: Option<u64>,
    built: Option<B>,
}

impl<B> Default for Kept<B> {
    fn default() -> Self {
        Self {
            revision: None,
            built: None,
        }
    }
}

impl<B> Kept<B> {
    /// Rebuild with `build` when `revision` is not the one last built from; returns
    /// whether it rebuilt. `build` returning `None` keeps nothing (an empty mesh).
    pub(crate) fn refresh(&mut self, revision: u64, build: impl FnOnce() -> Option<B>) -> bool {
        if self.revision == Some(revision) {
            return false;
        }
        self.revision = Some(revision);
        self.built = build();
        true
    }

    /// What was last built, if anything.
    pub(crate) fn get(&self) -> Option<&B> {
        self.built.as_ref()
    }
}

/// The gizmo's pipeline and its kept vertex buffer (with its vertex count).
pub(crate) struct NavMeshGizmo {
    pipeline: LazyPipeline,
    mesh: Kept<(wgpu::Buffer, u32)>,
}

impl NavMeshGizmo {
    /// The pipeline, built on its first draw: the forward `layout` (the handles
    /// pass's groups), positions only, both faces, alpha-blended into
    /// `output_format`, testing the view's depth without writing it.
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
            mesh: Kept::default(),
        }
    }

    /// The pipeline, for `Renderer::lazy_pipelines`.
    pub(crate) fn pipeline(&self) -> &LazyPipeline {
        &self.pipeline
    }
}

/// How many of `len` vertices fit a buffer of `max_bytes`, in whole quads (six
/// vertices): past it the rest of a huge navmesh is left out, not a failed upload.
pub(crate) fn vertices_that_fit(len: usize, max_bytes: u64) -> usize {
    let fit = (max_bytes / VERTEX_BYTES) as usize;
    len.min(fit - fit % 6)
}

impl Renderer {
    /// The shell hands over the baked navmesh (#1910): `revision` names the graph it
    /// comes from, and `triangles` (world space, three vertices a triangle) is called
    /// only when it differs from the last one handed over. Returns whether the
    /// vertex buffer was rebuilt.
    pub fn set_navmesh(&mut self, revision: u64, triangles: impl FnOnce() -> Vec<Vec3>) -> bool {
        let device = &self.device;
        self.navmesh.mesh.refresh(revision, || {
            let triangles = triangles();
            let count = vertices_that_fit(triangles.len(), device.limits().max_buffer_size);
            if count < triangles.len() {
                log::warn!("[Gizmos] the navmesh is too large to draw whole: part is left out");
            }
            let positions: Vec<[f32; 3]> =
                triangles[..count].iter().map(|p| p.to_array()).collect();
            (count > 0).then(|| {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("NavMesh Gizmo Vertices"),
                    contents: bytemuck::cast_slice(&positions),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                (buffer, count as u32)
            })
        })
    }

    /// The gizmo's slot of `draws`, while `gizmos` has it on and a navmesh is kept.
    pub(crate) fn precreate_navmesh(
        &self,
        gizmos: &SceneGizmos,
        draws: &mut OverlayDraws,
    ) -> Option<u32> {
        if !gizmos.on(Gizmo::NavMesh) || self.navmesh.mesh.get().is_none() {
            return None;
        }
        Some(draws.push(Mat4::IDENTITY, NAVMESH_TINT))
    }

    /// Draw the navmesh at `slot` into the handles pass, then put the line pipeline back.
    pub(super) fn draw_navmesh<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        bindings: &'a OverlayBindings,
        slot: Option<u32>,
    ) {
        let (Some(slot), Some((buffer, count))) = (slot, self.navmesh.mesh.get()) else {
            return;
        };
        pass.set_pipeline(self.navmesh.pipeline.get());
        pass.set_vertex_buffer(0, buffer.slice(..));
        bindings.bind(pass, slot);
        pass.draw(0..*count, 0..1);
        pass.set_pipeline(self.line_pipeline.get());
    }
}

#[cfg(test)]
#[path = "navmesh_tests.rs"]
mod tests;
