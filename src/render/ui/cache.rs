//! src/render/ui/cache.rs — a view's UI vertex buffers, one per canvas (#418).
//!
//! **Dirty strategy.** Each view keeps one vertex buffer per canvas plus the mesh it
//! holds. Rebuilding the CPU mesh is a cheap walk; a canvas's buffer is re-uploaded
//! only when its mesh differs from last frame's (a layout or graphic change), and
//! reallocated only when it outgrows its capacity. A static HUD therefore costs no
//! uploads per frame.

use std::collections::HashMap;

use glam::Vec2;

use super::effects::UiEffects;
use super::mesh::CanvasMesh;
use super::world::WorldUniforms;
use crate::ui::CanvasSpace;

/// One canvas's GPU copy of its mesh, and where it draws this frame.
pub(super) struct CanvasGpu {
    pub(super) mesh: CanvasMesh,
    pub(super) buffer: wgpu::Buffer,
    pub(super) place: CanvasPlace,
}

/// A view's UI vertex buffers, one per drawn canvas in draw order (see the module
/// docs for when they are re-uploaded).
#[derive(Default)]
pub struct UiViewCache {
    canvases: Vec<CanvasGpu>,
    /// How many canvas buffers the last frame (re-)uploaded — the dirty signal.
    pub(super) uploads: usize,
    /// How many canvases the last frame drew (a world canvas once per camera).
    pub(super) drawn: usize,
    /// The world canvases' per-camera uniforms (#429), made on first use.
    pub(super) world: Option<WorldUniforms>,
    /// Draw the screen-space canvases in an editor-mode (Scene view) render too —
    /// the Scene tab's UI overlay toggle (#423). Off: the Scene view shows only
    /// world canvases.
    pub screen_in_editor: bool,
    /// Clip slots, Mask textures and the backdrop blur (#426, #428).
    pub(super) effects: UiEffects,
}

impl UiViewCache {
    /// UI batches (draw calls) the last render of this view issued.
    pub fn last_batches(&self) -> usize {
        match self.drawn {
            0 => 0,
            _ => self.canvases.iter().map(|c| c.mesh.batches.len()).sum(),
        }
    }

    /// Canvas buffers (re-)uploaded by the last render of this view.
    pub fn last_uploads(&self) -> usize {
        self.uploads
    }

    /// Canvases the last render of this view drew (0 when it had nowhere to draw).
    pub fn last_drawn(&self) -> usize {
        self.drawn
    }

    /// Mask textures the last render drew (#428).
    pub fn last_mask_passes(&self) -> u32 {
        self.effects.mask_passes
    }

    /// Fullscreen backdrop-blur passes the last render ran (#426).
    pub fn last_blur_passes(&self) -> u32 {
        self.effects.blur_passes
    }

    /// Every synced canvas, in draw order.
    pub(super) fn canvases(&self) -> &[CanvasGpu] {
        &self.canvases
    }

    /// Synced canvas `i`, in draw order.
    pub(super) fn canvas(&self, i: usize) -> &CanvasGpu {
        &self.canvases[i]
    }

    /// Every synced canvas's placement, in draw order.
    pub(super) fn places(&self) -> impl Iterator<Item = CanvasPlace> + '_ {
        self.canvases.iter().map(|c| c.place)
    }

    /// Adopt this frame's `meshes`, uploading only the ones that changed.
    pub(super) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: Vec<(CanvasMesh, CanvasPlace)>,
    ) {
        let mut old: HashMap<u32, CanvasGpu> = self
            .canvases
            .drain(..)
            .map(|c| (c.mesh.canvas, c))
            .collect();
        self.uploads = 0;
        for (mesh, place) in meshes {
            let bytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
            let gpu = match old.remove(&mesh.canvas) {
                Some(c) if c.mesh == mesh => CanvasGpu { place, ..c },
                Some(c) if c.buffer.size() >= bytes.len() as u64 => {
                    queue.write_buffer(&c.buffer, 0, bytes);
                    self.uploads += 1;
                    CanvasGpu { mesh, place, ..c }
                }
                _ => {
                    self.uploads += 1;
                    let buffer = create_buffer(device, bytes);
                    CanvasGpu {
                        mesh,
                        buffer,
                        place,
                    }
                }
            };
            self.canvases.push(gpu);
        }
    }
}

/// A vertex buffer holding `bytes`, with room to grow (the next power of two).
fn create_buffer(device: &wgpu::Device, bytes: &[u8]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let mut contents = bytes.to_vec();
    contents.resize(bytes.len().next_power_of_two().max(256), 0);
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("UI Canvas Vertices"),
        contents: &contents,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    })
}

/// Where a synced canvas draws this frame — refreshed every frame, never part of
/// the re-upload check (a moving world canvas re-uploads nothing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasPlace {
    /// Its reference units' space.
    pub(crate) space: CanvasSpace,
    /// Its root rect's size, reference units.
    pub(crate) size: Vec2,
    /// The canvas entity's layer (camera culling).
    pub(crate) layer: u8,
}
