//! The static materials' base colours in one `texture_2d_array` a ray hit reads, since rusty
//! binds no texture array per material: each map at `SIZE` with mips, a layer a map.

use std::rc::{Rc, Weak};

use wgpu::util::DeviceExt;

use super::pass::RayPasses;
use crate::gpu::ops::CountedQueue as _;
use crate::gpu::textures::mips::view_format;
use crate::gpu::textures::{GpuTexture, TextureKind};
use crate::Renderer;
use rusty_scene::Scene;

/// `ssr_rays.wgsl`'s `NO_LAYER`.
const NO_LAYER: u32 = u32::MAX;
/// `ssr_rays.wgsl`'s `fs_resample` target.
const SIZE: u32 = 256;

pub(super) struct Albedo {
    pub view: wgpu::TextureView,
    pub layers: wgpu::Buffer,
    pub clamp: wgpu::Sampler,
    pub repeat: wgpu::Sampler,
    maps: Vec<Weak<GpuTexture>>,
    slots: Vec<u32>,
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
        label: Some("SSR Rays Base Colours"),
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

fn layers_buffer(device: &wgpu::Device, slots: &[u32]) -> wgpu::Buffer {
    let none = [NO_LAYER];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("SSR Rays Base Colour Layers"),
        contents: bytemuck::cast_slice(if slots.is_empty() { &none } else { slots }),
        usage: wgpu::BufferUsages::STORAGE,
    })
}

impl Albedo {
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

impl Renderer {
    /// The library's base colour maps a static material wears, resident, packed; and each
    /// material record's layer.
    pub(super) fn refresh_ray_albedo(&mut self, scene: &Scene) {
        let worn: Vec<_> = scene
            .materials
            .iter()
            .filter_map(|(name, m)| Some((name, m.base_color_map.as_ref()?)))
            .collect();
        let limit = self.device.limits().max_texture_array_layers as usize;
        let mut maps: Vec<_> = worn.iter().map(|(_, m)| *m).collect();
        maps.retain(|m| self.gpu_textures.contains_key(*m));
        maps.sort();
        maps.dedup();
        maps.truncate(limit);
        let textures: Vec<_> = maps.iter().map(|m| self.gpu_textures[*m].clone()).collect();
        let albedo = self
            .ssr_rays
            .albedo
            .get_or_insert_with(|| Albedo::new(&self.device));
        let passes = &self.post_passes.ssr.rays;
        albedo.pack((&self.device, &self.queue), passes, &textures);
        if let Some(texture) = albedo.texture_for_mips.take() {
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
        let mut slots = vec![NO_LAYER; records.table().records().0.len()];
        for (name, map) in worn {
            let Ok(layer) = maps.binary_search(&map) else {
                continue;
            };
            for (_, slot) in records.variants_of(scene.id(), name) {
                if let Some(at) = slots.get_mut(slot as usize) {
                    *at = layer as u32;
                }
            }
        }
        if slots != albedo.slots {
            albedo.layers = layers_buffer(&self.device, &slots);
            albedo.slots = slots;
        }
    }
}
