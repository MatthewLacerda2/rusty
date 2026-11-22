use crate::gpu::ops::CountedQueue as _;
use std::collections::HashMap;
use std::rc::Rc;

use glam::Vec2;

use super::custom::ShadeUniforms;
use super::effects::UiEffects;
use super::kept::{KeptLayout, KeptMeshes};
use super::mesh::CanvasMesh;
use super::world::WorldUniforms;
use rusty_ui::CanvasSpace;

pub(super) struct CanvasGpu {
    pub(super) mesh: Rc<CanvasMesh>,
    pub(super) buffer: wgpu::Buffer,
    pub(super) place: CanvasPlace,
}

#[derive(Default)]
pub struct UiViewCache {
    canvases: Vec<CanvasGpu>,
    pub(super) layout: KeptLayout,
    pub(super) kept: KeptMeshes,
    pub(super) uploads: usize,
    pub(super) drawn: usize,
    pub(super) world: Option<WorldUniforms>,
    pub screen_in_editor: bool,
    /// Target pixels `(x, y, w, h)`, top-left; `None`: the whole view. `rect_tool::OverlayFrame`
    /// maps the same box.
    pub screen_box: Option<(u32, u32, u32, u32)>,
    pub(super) effects: UiEffects,
    pub(super) shades: Option<ShadeUniforms>,
}

impl UiViewCache {
    pub fn last_batches(&self) -> usize {
        match self.drawn {
            0 => 0,
            _ => self.canvases.iter().map(|c| c.mesh.batches.len()).sum(),
        }
    }

    pub fn last_meshed(&self) -> usize {
        self.kept.built()
    }

    pub fn last_laid_out(&self) -> bool {
        self.layout.why().is_some()
    }

    pub fn last_relaid(&self) -> [u32; 7] {
        let why = self.layout.why();
        super::kept::Why::ALL.map(|w| u32::from(why == Some(w)))
    }

    /// Canvas buffers the last render (re-)uploaded. A test hook: `ui::draw_tests`.
    #[cfg(test)]
    pub fn last_uploads(&self) -> usize {
        self.uploads
    }

    /// Canvases the last render drew (0 with nowhere to draw). A test hook: `ui::draw_tests`.
    #[cfg(test)]
    pub fn last_drawn(&self) -> usize {
        self.drawn
    }

    pub fn last_mask_passes(&self) -> u32 {
        self.effects.mask_passes
    }

    pub fn last_blur_passes(&self) -> u32 {
        self.effects.blur_passes
    }

    pub(super) fn canvases(&self) -> &[CanvasGpu] {
        &self.canvases
    }

    pub(super) fn canvases_and_effects(&mut self) -> (&[CanvasGpu], &mut UiEffects) {
        (&self.canvases, &mut self.effects)
    }

    pub(super) fn canvas(&self, i: usize) -> &CanvasGpu {
        &self.canvases[i]
    }

    pub(super) fn places(&self) -> impl Iterator<Item = CanvasPlace> + '_ {
        self.canvases.iter().map(|c| c.place)
    }

    pub(super) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: Vec<(Rc<CanvasMesh>, CanvasPlace)>,
    ) {
        let mut old: HashMap<u32, CanvasGpu> = self
            .canvases
            .drain(..)
            .map(|c| (c.mesh.canvas, c))
            .collect();
        self.uploads = 0;
        for (mesh, place) in meshes {
            let gpu = match old.remove(&mesh.canvas) {
                // Adopt the new `Rc` even when equal, so the next still frame is `ptr_eq`.
                Some(c) if Rc::ptr_eq(&c.mesh, &mesh) || c.mesh == mesh => CanvasGpu {
                    mesh,
                    place,
                    buffer: c.buffer,
                },
                Some(c) if c.buffer.size() >= byte_len(&mesh) => {
                    queue.counted_write_buffer(&c.buffer, 0, bytemuck::cast_slice(&mesh.vertices));
                    self.uploads += 1;
                    CanvasGpu { mesh, place, ..c }
                }
                _ => {
                    self.uploads += 1;
                    let buffer = create_buffer(device, bytemuck::cast_slice(&mesh.vertices));
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

fn byte_len(mesh: &CanvasMesh) -> u64 {
    std::mem::size_of_val(mesh.vertices.as_slice()) as u64
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasPlace {
    pub(crate) space: CanvasSpace,
    pub(crate) size: Vec2,
    pub(crate) layer: u8,
}
