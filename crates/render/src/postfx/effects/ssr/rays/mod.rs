//! SSR's *Mixed* tracing (HDRP's): the march's misses traced against the static scene and
//! merged into its hits (`postfx/ssr_rays.wgsl`). Nothing here exists under `RayMarching`.

mod albedo;
mod pass;
mod targets;

pub(crate) use pass::{trace_and_merge, RayGroups, RayPasses};
pub(crate) use targets::RayTargets;

use crate::gpu::bind_entry as entry;
use crate::gpu::kept::Kept;
use crate::gpu::ops::CountedDevice as _;
use crate::postfx::params::PostSource;
use crate::Renderer;
use rusty_components::SsrTracing;
use rusty_core::settings::GraphicsSettings;
use rusty_scene::Scene;

type ShadeKey = ([wgpu::Buffer; 4], [wgpu::TextureView; 3]);

#[derive(Default)]
pub(crate) struct SsrRays {
    tree: Option<wgpu::BindGroup>,
    albedo: Option<albedo::Albedo>,
    shade: Kept<ShadeKey, wgpu::BindGroup>,
}

pub(crate) fn traces(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    let mixed = post
        .volume
        .is_some_and(|vc| vc.ssr.tracing == SsrTracing::Mixed);
    mixed && super::ssr_enabled(post, graphics, full)
}

impl Renderer {
    /// Before any pass records.
    pub(crate) fn begin_ssr_rays(&mut self, scene: &Scene, post: PostSource, editor_mode: bool) {
        let graphics = self.graphics(editor_mode);
        if !traces(post, graphics, true) {
            self.ssr_rays = SsrRays::default();
            return;
        }
        self.ssr_rays.tree = self.take_rays(scene);
        if self.ssr_rays.tree.is_some() {
            self.refresh_ray_albedo(scene);
        }
    }

    pub(crate) fn end_ssr_rays(&mut self) {
        self.ssr_rays.tree = None;
        self.release_rays();
    }

    /// `None` under `RayMarching` or a refused tree.
    pub(crate) fn ssr_ray_groups(&mut self) -> Option<(wgpu::BindGroup, wgpu::BindGroup)> {
        let tree = self.ssr_rays.tree.clone()?;
        let albedo = self.ssr_rays.albedo.as_ref()?;
        let (objects, materials) = self.draw_buffers.shade_buffers();
        let lightmaps = self.lightmaps.binding();
        let key = (
            [
                self.lighting_buffer.clone(),
                objects.clone(),
                materials.clone(),
                albedo.layers.clone(),
            ],
            [
                lightmaps.pages.clone(),
                lightmaps.masks.clone(),
                albedo.view.clone(),
            ],
        );
        let layout = &self.post_passes.ssr.rays.shade;
        let device = &self.device;
        let shade = self.ssr_rays.shade.get(key.clone(), || {
            let ([lighting, objects, materials, layers], [pages, masks, colours]) = &key;
            let view = |v| wgpu::BindingResource::TextureView(v);
            let sampler = |s| wgpu::BindingResource::Sampler(s);
            device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSR Rays Shade Bind Group"),
                layout,
                entries: &[
                    entry(0, lighting.as_entire_binding()),
                    entry(1, objects.as_entire_binding()),
                    entry(2, materials.as_entire_binding()),
                    entry(3, view(pages)),
                    entry(4, view(masks)),
                    entry(5, sampler(&albedo.clamp)),
                    entry(6, view(colours)),
                    entry(7, layers.as_entire_binding()),
                    entry(8, sampler(&albedo.repeat)),
                ],
            })
        });
        Some((tree, shade.clone()))
    }
}

#[cfg(test)]
impl Renderer {
    pub(crate) fn holds_ssr_rays(&self) -> bool {
        self.ssr_rays.tree.is_some() || self.ssr_rays.albedo.is_some()
    }
}
