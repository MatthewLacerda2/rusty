use crate::gpu::ops::CountedDevice as _;
use std::sync::LazyLock;

use super::{composite_layout, Composite, PostEffect};
use crate::gpu::bind_entry as entry;
use crate::gpu::layout_entries::texture_entry;
use crate::postfx::PostFx;

pub(crate) struct CompositeTexture {
    pub name: &'static str,
    pub view: fn(&PostFx) -> Option<&wgpu::TextureView>,
}

pub(crate) struct PlacedTexture {
    pub effect: &'static PostEffect,
    pub function: &'static Composite,
    pub texture: &'static CompositeTexture,
}

impl PlacedTexture {
    pub(crate) fn of(function: &Composite) -> impl Iterator<Item = &'static PlacedTexture> + '_ {
        (composite_textures().iter()).filter(move |t| std::ptr::eq(t.function, function))
    }

    pub(crate) fn wgsl_name(&self) -> String {
        format!("{}_{}", self.effect.name, self.texture.name)
    }
}

pub(crate) const GROUP: u32 = 1;

pub(crate) fn composite_textures() -> &'static [PlacedTexture] {
    static TEXTURES: LazyLock<Vec<PlacedTexture>> = LazyLock::new(|| {
        composite_layout()
            .iter()
            .flat_map(|p| {
                (p.function.textures.iter()).map(|texture| PlacedTexture {
                    effect: p.effect,
                    function: p.function,
                    texture,
                })
            })
            .collect()
    });
    &TEXTURES
}

pub(crate) struct CompositeTextures {
    pub(crate) layout: wgpu::BindGroupLayout,
    fallback: wgpu::TextureView,
}

impl CompositeTextures {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let entries: Vec<_> = (0..composite_textures().len() as u32)
            .map(|binding| texture_entry(binding, wgpu::TextureViewDimension::D2))
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PostFX Effect Textures Layout"),
            entries: &entries,
        });
        Self {
            layout,
            fallback: black(device),
        }
    }

    pub(crate) fn black(&self) -> &wgpu::TextureView {
        &self.fallback
    }

    pub(crate) fn bind_group(&self, device: &wgpu::Device, pfx: &PostFx) -> wgpu::BindGroup {
        let views = bound_views(pfx, &self.fallback);
        let entries: Vec<_> = (views.iter().enumerate())
            .map(|(i, view)| entry(i as u32, wgpu::BindingResource::TextureView(view)))
            .collect();
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PostFX Effect Textures"),
            layout: &self.layout,
            entries: &entries,
        })
    }
}

pub(super) fn bound_views<'a>(
    pfx: &'a PostFx,
    fallback: &'a wgpu::TextureView,
) -> Vec<&'a wgpu::TextureView> {
    (composite_textures().iter())
        .map(|p| (p.texture.view)(pfx).unwrap_or(fallback))
        .collect()
}

fn black(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("PostFX No Effect Texture"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&Default::default())
}
