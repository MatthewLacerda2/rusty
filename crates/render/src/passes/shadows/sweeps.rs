use super::cascades::Cascade;
use super::casters::{CasterFrame, LightVolume, Sweep};
use super::ShadowRenderer;
use crate::gpu::ops::CountedEncoder as _;
use crate::lod::LodSelection;
use crate::timing::GpuPass;

impl ShadowRenderer {
    pub(super) fn render_cascades(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) {
        if self.cascades.is_empty() {
            return;
        }
        let scene = frame.scene;
        let statics = frame.statics;
        let with_statics = self.sun_statics;
        let key = |c: &Cascade| Some((statics, c.light_space, with_statics));
        let stale: Vec<usize> = (0..self.cascades.len())
            .filter(|&i| self.static_cache[i] != key(&self.cascades[i]))
            .collect();
        if !stale.is_empty() {
            // The bake outlives the frame, so it cannot follow the camera's LOD: every group
            // at LOD0.
            let finest = LodSelection::finest(scene);
            let batches = match with_statics {
                true => {
                    let volumes: Vec<LightVolume> = stale
                        .iter()
                        .map(|&i| LightVolume::open(self.cascades[i].light_space))
                        .collect();
                    self.prepare_casters(frame, &finest, Sweep::Static, &volumes)
                }
                false => stale.iter().map(|_| Vec::new()).collect(),
            };
            for (&i, batches) in stale.iter().zip(&batches) {
                let view = &self.static_layers[i];
                let mut pass = depth_pass(encoder, frame, "Shadow Static Pass", view, true);
                self.draw_casters(&mut pass, frame, batches, Sweep::Static, i);
            }
            for i in stale {
                self.static_cache[i] = key(&self.cascades[i]);
            }
        }

        self.copy_static_layers(encoder);

        let all = self.cascades.iter();
        let all: Vec<LightVolume> = all.map(|c| LightVolume::open(c.light_space)).collect();
        let batches = self.prepare_casters(frame, lod, Sweep::Dynamic, &all);
        for (i, batches) in batches.iter().enumerate() {
            if batches.is_empty() {
                continue;
            }
            let view = &self.active_layers[i];
            let mut pass = depth_pass(encoder, frame, "Shadow Dynamic Pass", view, false);
            self.draw_casters(&mut pass, frame, batches, Sweep::Dynamic, i);
        }
    }

    fn copy_static_layers(&self, encoder: &mut wgpu::CommandEncoder) {
        let layer = |texture| wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        encoder.copy_texture_to_texture(
            layer(&self.static_texture),
            layer(&self.active_texture),
            wgpu::Extent3d {
                width: Self::CASCADE_SIZE,
                height: Self::CASCADE_SIZE,
                depth_or_array_layers: self.cascades.len() as u32,
            },
        );
    }
}

pub(super) fn depth_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    frame: &CasterFrame,
    label: &str,
    view: &'a wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    let load = if clear {
        wgpu::LoadOp::Clear(1.0)
    } else {
        wgpu::LoadOp::Load
    };
    encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some(label),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: frame.timer.writes(GpuPass::Shadows),
        occlusion_query_set: None,
    })
}
