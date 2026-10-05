//! Baked lightmaps on the GPU (#438): the scene's atlas pages as one 2D texture array
//! at group 0, binding 10, with a clamped linear sampler at 11. Each lightmapped
//! instance carries its page and the scale/offset into it, so lightmapped copies of a
//! prop still draw as one instanced call (Unity's lightmap index + scale/offset).
//!
//! Pages are RGBM PNGs (`scene::lighting::lightmap::encode`), uploaded *linear* as
//! `Rgba8Unorm`: the shader decodes RGBM itself. An array is built once per distinct
//! page list and cached, so two scenes alternating in one frame cost a lookup, not a
//! reload. A rebake writes changed pages under new names (their file name hashes the
//! texels), so the cache never serves a stale page. A page that fails to load, or
//! pages of different sizes, leave the scene unlit by lightmaps (probe / ambient
//! fallback) rather than half-bound.
//!
//! A directional bake (#810) adds its direction pages as a second array at binding 16,
//! read through the same sampler with the same page index and UV. They are bound only
//! beside a bound colour array whose page count and size they match; otherwise the
//! zeroed fallback stands in, whose zero directionality leaves the lightmap as is.

use std::collections::HashMap;
use std::rc::Rc;

/// One uploaded page array.
pub(crate) struct LightmapArray {
    /// Page edge and page count, to match direction pages to colour pages.
    shape: (u32, u32),
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

/// What group 0 binds for lightmaps, and the arrays built so far.
pub(crate) struct Lightmaps {
    /// The colour and direction page lists bound now (empty: the fallback).
    bound: (Vec<String>, Vec<String>),
    current: Option<Rc<LightmapArray>>,
    directions: Option<Rc<LightmapArray>>,
    /// Arrays by page list, colour and direction alike.
    cache: HashMap<Vec<String>, Option<Rc<LightmapArray>>>,
    /// A 1×1×1 black array, bound when the scene has no lightmaps.
    fallback: LightmapArray,
}

impl Lightmaps {
    /// No lightmaps bound yet. The fallback needs no upload: a new texture is zeroed,
    /// and an all-zero RGBM texel decodes to black.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let fallback = create(device, 1, 1);
        Self {
            bound: (Vec::new(), Vec::new()),
            current: None,
            directions: None,
            cache: HashMap::new(),
            fallback,
        }
    }

    /// Bind the arrays of `pages` and their `directions` (#810; empty for a
    /// non-directional bake), loading each on first sight. Returns whether the
    /// binding changed, so the caller rebuilds group 0.
    pub(crate) fn bind(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pages: &[String],
        directions: &[String],
    ) -> bool {
        if self.bound.0 == pages && self.bound.1 == directions {
            return false;
        }
        self.bound = (pages.to_vec(), directions.to_vec());
        self.current = self.array(device, queue, pages);
        let shape = self.current.as_ref().map(|a| a.shape);
        self.directions = self
            .array(device, queue, directions)
            .filter(|d| Some(d.shape) == shape);
        true
    }

    /// The (cached) array of `pages`; `None` for no pages or one that won't load.
    fn array(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pages: &[String],
    ) -> Option<Rc<LightmapArray>> {
        if pages.is_empty() {
            return None;
        }
        self.cache
            .entry(pages.to_vec())
            .or_insert_with(|| load(device, queue, pages).map(Rc::new))
            .clone()
    }

    /// Whether a lightmap array is bound: when not, instances must not read it.
    pub(crate) fn resident(&self) -> bool {
        self.current.is_some()
    }

    /// Whether direction pages are bound beside it (#810).
    pub(crate) fn directional(&self) -> bool {
        self.directions.is_some()
    }

    /// The bound colour and direction views and their sampler (the fallback's when
    /// none is).
    pub(crate) fn binding(&self) -> (&wgpu::TextureView, &wgpu::TextureView, &wgpu::Sampler) {
        let array = self.current.as_deref().unwrap_or(&self.fallback);
        let directions = self.directions.as_deref().unwrap_or(&self.fallback);
        (&array.view, &directions.view, &array.sampler)
    }
}

/// Read every page; `None` (with a warning) when one is unreadable or the sizes differ.
fn load(device: &wgpu::Device, queue: &wgpu::Queue, pages: &[String]) -> Option<LightmapArray> {
    let mut size = None;
    let mut layers = Vec::with_capacity(pages.len());
    for path in pages {
        let image = match image::open(path) {
            Ok(img) => img.to_rgba8(),
            Err(e) => {
                log::warn!("[Lightmap] cannot load page {path}: {e}");
                return None;
            }
        };
        let edge = image.width();
        if image.height() != edge || size.is_some_and(|s| s != edge) {
            log::warn!("[Lightmap] page {path} is not the atlas's square size");
            return None;
        }
        size = Some(edge);
        layers.push(image.into_raw());
    }
    Some(upload(device, queue, size?, &layers))
}

/// Upload `layers` (each `size`² RGBA8 texels) as a linear 2D texture array.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: u32,
    layers: &[Vec<u8>],
) -> LightmapArray {
    let array = create(device, size, layers.len() as u32);
    for (layer, texels) in layers.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &array.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }
    array
}

/// A zeroed `size`² linear RGBA8 array of `layers` pages, its view and its sampler.
fn create(device: &wgpu::Device, size: u32, layers: u32) -> LightmapArray {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Lightmap Pages"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Lightmap Pages"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Lightmap Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    LightmapArray {
        shape: (size, layers),
        texture,
        view,
        sampler,
    }
}
