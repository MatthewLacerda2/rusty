//! src/render/ui/custom/uniforms.rs — the per-graphic `UiShade` uniforms (#427).
//!
//! Each view packs one aligned slot per custom-shaded batch per frame, after its
//! meshes sync: the graphic's rect, the UI clock and its runtime params packed by the
//! variant's layout (#399). The clock moves every frame, so the slots are always
//! rewritten — but a param change is only ever a buffer write, never a rebuild.

use std::collections::HashMap;

use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::{RenderView, Renderer};
use crate::shadergen::params::PackedParams;

/// One graphic's slot — matches the variant's `UiShade` (`params::UI_UNIFORM_DECL`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct UiShadeUniform {
    /// `xy` the rect's origin, `zw` its x axis (canvas NDC).
    origin_x: [f32; 4],
    /// `xy` the rect's y axis (canvas NDC), `zw` its size in pixels.
    axis_y_size: [f32; 4],
    /// `x` the UI clock, seconds.
    time: [f32; 4],
    v: PackedParams,
}

/// A view's shade uniforms for this frame: one slot per shaded batch whose variant
/// built, by `(canvas index, batch index)`.
pub(crate) struct ShadeUniforms {
    buffer: GrowBuffer,
    pub(super) group: wgpu::BindGroup,
    pub(super) offsets: Offsets,
}

/// Slot offsets by `(canvas index, batch index)`.
type Offsets = HashMap<(usize, usize), u32>;

/// One slot's size in bytes.
pub(super) const SLOT: u64 = std::mem::size_of::<UiShadeUniform>() as u64;

impl Renderer {
    /// Pack `view`'s shaded batches into its shade uniforms at clock `time`, building
    /// each named variant on first use. Runs after the meshes sync, every frame.
    pub(in crate::render::ui) fn prepare_ui_shades(&mut self, view: &mut RenderView, time: f32) {
        let (bytes, offsets) = self.pack_ui_shades(view, time);
        if offsets.is_empty() {
            if let Some(u) = &mut view.ui.shades {
                u.offsets.clear();
            }
            return;
        }
        self.upload_ui_shades(view, &bytes, offsets);
    }

    /// One slot per shaded batch whose variant builds, and where each sits.
    fn pack_ui_shades(&mut self, view: &RenderView, time: f32) -> (Vec<u8>, Offsets) {
        let shaders = &mut self.ui_renderer.shaders;
        shaders.refresh();
        let align = self.device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = (SLOT as usize).next_multiple_of(align);
        let (mut bytes, mut offsets) = (Vec::new(), HashMap::new());
        for (c, canvas) in view.ui.canvases().iter().enumerate() {
            for (b, batch) in canvas.mesh.batches.iter().enumerate() {
                let Some(shade) = &batch.shade else { continue };
                let Some(variant) = shaders.variant(&self.device, &shade.shader.name) else {
                    continue;
                };
                let [o, x, y] = shade.rect;
                let u = UiShadeUniform {
                    origin_x: [o[0], o[1], x[0], x[1]],
                    axis_y_size: [y[0], y[1], shade.size.x, shade.size.y],
                    time: [time, 0.0, 0.0, 0.0],
                    v: variant.params.pack(&shade.shader.params),
                };
                offsets.insert((c, b), bytes.len() as u32);
                bytes.extend_from_slice(bytemuck::bytes_of(&u));
                bytes.resize(bytes.len() + stride - SLOT as usize, 0);
            }
        }
        (bytes, offsets)
    }

    /// Write `bytes` into the view's shade buffer, (re)binding it when it is new or grew.
    fn upload_ui_shades(&mut self, view: &mut RenderView, bytes: &[u8], offsets: Offsets) {
        let layout = &self.ui_renderer.shaders.shade_layout;
        let bind = |buffer: &wgpu::Buffer| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("UI Shade Uniforms"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(SLOT),
                    }),
                }],
            })
        };
        match &mut view.ui.shades {
            Some(u) => {
                if u.buffer.upload(&self.device, &self.queue, bytes) {
                    u.group = bind(u.buffer.buffer());
                }
                u.offsets = offsets;
            }
            None => {
                let usage = wgpu::BufferUsages::UNIFORM;
                let mut buffer = GrowBuffer::new(&self.device, "UI Shade Uniforms", usage);
                buffer.upload(&self.device, &self.queue, bytes);
                let group = bind(buffer.buffer());
                view.ui.shades = Some(ShadeUniforms {
                    buffer,
                    group,
                    offsets,
                });
            }
        }
    }
}
