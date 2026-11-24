//! SSR's *Mixed* tracing (HDRP's): the march's misses traced against the static scene and
//! merged into its hits (`postfx/ssr_rays.wgsl`). Nothing here exists under `RayMarching`.

mod maps;
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
use rusty_scene::{Scene, SceneId};

type ShadeKey = ([wgpu::Buffer; 4], [wgpu::TextureView; 3]);

/// What *Mixed* built, kept for the scene that traces (`owner`) across renders that do not
/// (the editor's Inspector preview renders a second scene each frame); freed when the owner
/// stops tracing or leaves the retained scenes.
#[derive(Default)]
pub(crate) struct SsrRays {
    tree: Option<wgpu::BindGroup>,
    owner: Option<SceneId>,
    maps: Option<maps::HitMaps>,
    shade: Kept<ShadeKey, wgpu::BindGroup>,
    /// Map arrays built from nothing, for the tests.
    #[cfg(test)]
    maps_builds: u32,
}

impl SsrRays {
    fn free(&mut self) {
        self.owner = None;
        self.maps = None;
        self.shade = Kept::default();
    }
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
        self.ssr_rays.tree = None;
        let owner = self.ssr_rays.owner;
        let owner_gone = owner.is_some_and(|id| self.retained.scene(id).is_none());
        if !traces(post, graphics, true) {
            if owner == Some(scene.id()) || owner_gone {
                self.ssr_rays.free();
            }
            return;
        }
        self.ssr_rays.owner = Some(scene.id());
        self.ssr_rays.tree = self.take_rays(scene);
        if self.ssr_rays.tree.is_some() {
            self.refresh_ray_maps(scene);
        }
    }

    pub(crate) fn end_ssr_rays(&mut self) {
        self.ssr_rays.tree = None;
        self.release_rays();
    }

    /// `None` under `RayMarching` or a refused tree.
    pub(crate) fn ssr_ray_groups(&mut self) -> Option<(wgpu::BindGroup, wgpu::BindGroup)> {
        let tree = self.ssr_rays.tree.clone()?;
        let maps = self.ssr_rays.maps.as_ref()?;
        let (objects, materials) = self.draw_buffers.shade_buffers();
        let lightmaps = self.lightmaps.binding();
        let key = (
            [
                self.lighting_buffer.clone(),
                objects.clone(),
                materials.clone(),
                maps.layers.clone(),
            ],
            [
                lightmaps.pages.clone(),
                lightmaps.masks.clone(),
                maps.view.clone(),
            ],
        );
        let layout = &self.post_passes.ssr.rays.shade;
        let device = &self.device;
        let shade = self.ssr_rays.shade.get(key.clone(), || {
            let ([lighting, objects, materials, layers], [pages, masks, hit_maps]) = &key;
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
                    entry(5, sampler(&maps.clamp)),
                    entry(6, view(hit_maps)),
                    entry(7, layers.as_entire_binding()),
                    entry(8, sampler(&maps.repeat)),
                ],
            })
        });
        Some((tree, shade.clone()))
    }
}

#[cfg(test)]
impl Renderer {
    pub(crate) fn holds_ssr_rays(&self) -> bool {
        self.ssr_rays.tree.is_some() || self.ssr_rays.maps.is_some()
    }

    pub(crate) fn ssr_maps_builds(&self) -> u32 {
        self.ssr_rays.maps_builds
    }
}
