//! The NavMesh gizmo (#1910): the baked navmesh as translucent cyan over the floor
//! in the Scene view, under the `NavMesh` gizmo flag, Unity's navmesh overlay.
//!
//! **Render never sees the graph.** `render` may not import `navigation`, so the
//! shell turns the graph into world-space triangles (one quad per walkable span, on
//! the span's floor plane) and hands them over with a revision through
//! [`Renderer::set_navmesh`]. Render only uploads them.
//!
//! **Patched where the graph changed, never rebuilt per frame.** The other overlays
//! rebuild their buffers every frame; a Dust2-sized navmesh is hundreds of thousands
//! of quads, so it is kept as one vertex buffer per tile of cells, and the shell
//! hands over ([`NavMeshPatch`]) only the tiles a rebake touched: dragging a crate
//! re-uploads the few tiles around it. A frame on which the graph did not change
//! uploads nothing; a new graph or a re-shaped grid starts over.
//!
//! **Drawn translucent.** Its own pipeline over `engine/shaders/gizmos/navmesh.wgsl`:
//! a bare 12-byte position per vertex, alpha-blended, depth-tested against the
//! scene without writing it, after the opaque probe balls and before the lines, so
//! the grid and the collider wires stay crisp over it.

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::{OverlayBindings, OverlayDraws};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::DEPTH_FORMAT;
use crate::Renderer;
use rusty_core::application::{Gizmo, SceneGizmos};

/// Unity's navmesh blue-cyan, translucent so the floor reads through it.
pub(crate) const NAVMESH_TINT: [f32; 4] = [0.2, 0.8, 1.0, 0.4];

/// One vertex's bytes: a world position.
const VERTEX_BYTES: u64 = 12;

/// The graph a navmesh was handed from, exactly: which instance (its address) and
/// its grid's shape, so a replaced graph or a re-shaped grid reads as new whatever
/// its generation says. Render cannot name the graph, so the shell fills it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavMeshSource {
    pub instance: usize,
    pub width: i32,
    pub height: i32,
    /// The grid's `min_x`, `min_z` and spacing, as bits.
    pub grid: [u32; 3],
}

/// What the kept buffers show: a source at one bake generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavMeshRevision {
    pub source: NavMeshSource,
    pub generation: u64,
}

/// A rewrite of the kept tiles: `reset` (a tile count) drops every tile first and
/// starts over; `tiles` are the tiles rebuilt, each with its world-space
/// triangles (three vertices a triangle). The tiling is the shell's.
pub struct NavMeshPatch<I> {
    pub revision: NavMeshRevision,
    pub reset: Option<usize>,
    pub tiles: I,
}

/// The gizmo's pipeline and its kept vertex buffers, one per tile (with its vertex
/// count; `None`: nothing walkable there).
pub(crate) struct NavMeshGizmo {
    pipeline: LazyPipeline,
    revision: Option<NavMeshRevision>,
    tiles: Vec<Option<(wgpu::Buffer, u32)>>,
    /// Whether the "too large to draw whole" warning was given (it is given once).
    warned: bool,
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
            revision: None,
            tiles: Vec::new(),
            warned: false,
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
    /// What the kept navmesh shows, for the shell to diff against (#1910).
    pub fn navmesh_revision(&self) -> Option<NavMeshRevision> {
        self.navmesh.revision
    }

    /// Apply the shell's `patch` to the kept navmesh: upload only the tiles it
    /// names. Returns the bytes uploaded.
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

    /// The gizmo's slot of `draws`, while `gizmos` has it on and a navmesh is kept.
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

    /// Draw the navmesh at `slot` into the handles pass, then put the line pipeline back.
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
