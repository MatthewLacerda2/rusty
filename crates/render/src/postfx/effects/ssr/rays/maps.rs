//! The static materials' base colour and metallic maps in one array a ray hit reads, a
//! layer a map at `SIZE` with mips. A metallic texel's sRGB round trip keeps it within an
//! 8-bit step.

use std::rc::{Rc, Weak};

use wgpu::util::DeviceExt;

use super::pass::RayPasses;
use crate::gpu::ops::CountedQueue as _;
use crate::gpu::textures::mips::view_format;
use crate::gpu::textures::{GpuTexture, TextureKind};
use crate::Renderer;
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_scene::Scene;

/// `ssr_rays.wgsl`'s `NO_LAYER`.
const NO_LAYER: u32 = u32::MAX;
/// `ssr_rays.wgsl`'s `fs_resample` target.
const SIZE: u32 = 256;

/// In `rays_map_layers`' order; sorted first, a base colour keeps its layer past the limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    BaseColour,
    Metallic,
}

impl Role {
    const ALL: [Role; 2] = [Role::BaseColour, Role::Metallic];

    fn map(self, m: &MaterialAsset) -> Option<&MapRef> {
        match self {
            Role::BaseColour => m.base_color_map.as_ref(),
            Role::Metallic => m.metallic_map.as_ref(),
        }
    }

    fn kind(self) -> TextureKind {
        match self {
            Role::BaseColour => TextureKind::Colour,
            Role::Metallic => TextureKind::Mask,
        }
    }
}

pub(super) struct HitMaps {
    pub view: wgpu::TextureView,
    pub layers: wgpu::Buffer,
    pub clamp: wgpu::Sampler,
    pub repeat: wgpu::Sampler,
    maps: Vec<Weak<GpuTexture>>,
    slots: Vec<[u32; 2]>,
    /// Packed, its mips still to make (`MipGenerator` is the renderer's).
    texture_for_mips: Option<wgpu::Texture>,
}

fn sampler(device: &wgpu::Device, mode: wgpu::AddressMode) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("SSR Rays Sampler"),
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

fn array(device: &wgpu::Device, layers: u32) -> wgpu::Texture {
    use wgpu::TextureUsages as U;
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSR Rays Maps"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: layers.max(1),
        },
        mip_level_count: SIZE.ilog2() + 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: view_format(TextureKind::Colour),
        usage: U::TEXTURE_BINDING | U::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn layers_buffer(device: &wgpu::Device, slots: &[[u32; 2]]) -> wgpu::Buffer {
    let none = [[NO_LAYER; 2]];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("SSR Rays Map Layers"),
        contents: bytemuck::cast_slice(if slots.is_empty() { &none } else { slots }),
        usage: wgpu::BufferUsages::STORAGE,
    })
}

impl HitMaps {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let view = array(device, 1).create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Self {
            view,
            layers: layers_buffer(device, &[]),
            clamp: crate::gpu::linear_clamp(device),
            repeat: sampler(device, wgpu::AddressMode::Repeat),
            maps: Vec::new(),
            slots: Vec::new(),
            texture_for_mips: None,
        }
    }

    /// Packs `maps` again when they are not the ones packed: never per frame.
    fn pack(
        &mut self,
        gpu: (&wgpu::Device, &wgpu::Queue),
        passes: &RayPasses,
        maps: &[Rc<GpuTexture>],
    ) {
        let same = |(a, b): (&Weak<GpuTexture>, &Rc<GpuTexture>)| a.ptr_eq(&Rc::downgrade(b));
        if self.maps.len() == maps.len() && self.maps.iter().zip(maps).all(same) {
            return;
        }
        let (device, queue) = gpu;
        let texture = array(device, maps.len() as u32);
        let mut encoder = device.create_command_encoder(&Default::default());
        for (layer, map) in (0..).zip(maps) {
            let target = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            });
            passes.resample(device, &mut encoder, (&map.view, &self.clamp), &target);
        }
        queue.counted_submit([encoder.finish()]);
        self.view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        self.maps = maps.iter().map(Rc::downgrade).collect();
        self.texture_for_mips = Some(texture);
    }
}

/// Keeps the first `limit` resident `keys`, returning their textures.
fn layered<K: Copy, T>(
    keys: &mut Vec<K>,
    limit: usize,
    resident: impl Fn(K) -> Option<T>,
) -> Vec<T> {
    let mut textures = Vec::with_capacity(keys.len().min(limit));
    keys.retain(|&key| match resident(key) {
        Some(t) if textures.len() < limit => {
            textures.push(t);
            true
        }
        _ => false,
    });
    textures
}

impl Renderer {
    /// The resident maps a static material wears, packed; each material record's layers.
    pub(super) fn refresh_ray_maps(&mut self, scene: &Scene) {
        let worn: Vec<_> = scene
            .materials
            .iter()
            .flat_map(|(name, m)| Role::ALL.map(|role| Some((name, (role, role.map(m)?)))))
            .flatten()
            .collect();
        let limit = self.device.limits().max_texture_array_layers as usize;
        let mut keys: Vec<_> = worn.iter().map(|&(_, key)| key).collect();
        keys.sort();
        keys.dedup();
        let resident = |(role, map): (Role, &MapRef)| self.texture_for(map, role.kind());
        let textures = layered(&mut keys, limit, resident);
        let rays = &mut self.ssr_rays;
        if rays.maps.is_none() {
            #[cfg(test)]
            {
                rays.maps_builds += 1;
            }
            rays.maps = Some(HitMaps::new(&self.device));
        }
        let maps = rays.maps.as_mut().expect("made above");
        let passes = &self.post_passes.ssr.rays;
        maps.pack((&self.device, &self.queue), passes, &textures);
        if let Some(texture) = maps.texture_for_mips.take() {
            for layer in 0..texture.depth_or_array_layers() {
                self.mip_generator.generate_layer(
                    &self.device,
                    &self.queue,
                    (&texture, layer),
                    TextureKind::Colour,
                );
            }
        }
        let records = &self.retained.records;
        let mut slots = vec![[NO_LAYER; 2]; records.table().records().0.len()];
        for (name, key) in worn {
            let Ok(layer) = keys.binary_search(&key) else {
                continue;
            };
            for (_, slot) in records.variants_of(scene.id(), name) {
                if let Some(at) = slots.get_mut(slot as usize) {
                    at[key.0 as usize] = layer as u32;
                }
            }
        }
        if slots != maps.slots {
            maps.layers = layers_buffer(&self.device, &slots);
            maps.slots = slots;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{layered, Role};

    #[test]
    fn past_the_layer_limit_a_metallic_map_goes_without_before_a_base_colour_map() {
        let mut keys = vec![
            (Role::Metallic, "a"),
            (Role::BaseColour, "z"),
            (Role::Metallic, "b"),
            (Role::BaseColour, "missing"),
        ];
        keys.sort();
        let resident = |(_, name): (Role, &'static str)| (name != "missing").then_some(name);
        let textures = layered(&mut keys, 2, resident);
        assert_eq!(textures, ["z", "a"], "a map not resident takes no layer");
        assert_eq!(keys, [(Role::BaseColour, "z"), (Role::Metallic, "a")]);
    }
}
