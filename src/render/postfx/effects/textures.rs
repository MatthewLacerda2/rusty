//! The composite's textures of an effect's own (#1473): a composite function
//! declares the textures it reads ([`CompositeTexture`]), and the registry lays them
//! out in a bind group of their own, group 1 of the composite, one binding each in
//! run order ([`composite_textures`]). The assembled composite declares them and
//! passes each to its function as an argument (`effects::assemble`), so neither
//! `postfx/frame.wgsl`, the chain's io layout nor [`PostFx::run`] names one. A
//! texture its effect has none of this frame is bound to one 1×1 black texel,
//! shared by every effect ([`CompositeTextures`]).

use std::sync::LazyLock;

use super::{composite_layout, Composite, PostEffect};
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::layout_entries::texture_entry;
use crate::render::postfx::PostFx;

/// A 2D float texture a composite function reads, sampled with the chain's own
/// samplers (`postfx::frame`'s `s_color`, linear clamp, or `s_skybox`, wrapping).
pub(crate) struct CompositeTexture {
    /// Its name within the effect: the composite declares it as `<effect>_<name>`.
    pub name: &'static str,
    /// This frame's view: an effect's target (a slot of [`PostFx::targets`]) or a
    /// texture it picked. `None` binds the black texel.
    pub view: fn(&PostFx) -> Option<&wgpu::TextureView>,
}

/// One declared texture in its place: its binding in group 1 is its index.
pub(crate) struct PlacedTexture {
    pub effect: &'static PostEffect,
    /// The function that reads it.
    pub function: &'static Composite,
    pub texture: &'static CompositeTexture,
}

impl PlacedTexture {
    /// The textures `function` reads, in the order it takes them.
    pub(crate) fn of(function: &Composite) -> impl Iterator<Item = &'static PlacedTexture> + '_ {
        (composite_textures().iter()).filter(move |t| std::ptr::eq(t.function, function))
    }

    /// Its name in the assembled composite.
    pub(crate) fn wgsl_name(&self) -> String {
        format!("{}_{}", self.effect.name, self.texture.name)
    }
}

/// The composite's group for effects' textures.
pub(crate) const GROUP: u32 = 1;

/// Every declared texture, in the composite's run order (binding `i` is the `i`th).
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

/// The layout of group 1 and the black texel a texture without a view binds.
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

    /// Group 1 for this frame: each declared texture's view, else the black texel.
    pub(crate) fn bind_group(&self, device: &wgpu::Device, pfx: &PostFx) -> wgpu::BindGroup {
        let views = bound_views(pfx, &self.fallback);
        let entries: Vec<_> = (views.iter().enumerate())
            .map(|(i, view)| entry(i as u32, wgpu::BindingResource::TextureView(view)))
            .collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PostFX Effect Textures"),
            layout: &self.layout,
            entries: &entries,
        })
    }
}

/// The view bound at each binding of group 1: one per declared texture, in order.
pub(super) fn bound_views<'a>(
    pfx: &'a PostFx,
    fallback: &'a wgpu::TextureView,
) -> Vec<&'a wgpu::TextureView> {
    (composite_textures().iter())
        .map(|p| (p.texture.view)(pfx).unwrap_or(fallback))
        .collect()
}

/// The 1×1 black texel (wgpu zero-fills a new texture): an effect reading it adds
/// nothing, so an effect without its texture draws as if absent.
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
