use crate::gpu::ops::CountedDevice as _;
use std::rc::Rc;

use super::map_slots::{
    all_bindings, map_kind, map_sampler_binding, material_texture_paths, MAP_SLOTS,
};
use crate::gpu::material_cache::{MapPaths, ParamOwner, MATERIAL_MAPS, MATERIAL_TEXTURES};
use crate::gpu::textures::deep::{DEEP_FORMAT, SIGNED_NORMAL_MAP};
use crate::gpu::textures::TextureKind;
use crate::{GpuTexture, MaterialData, Renderer};
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_scene::shader_overrides::ParamValues;
use rusty_scene::{Scene, SceneId};

pub(crate) type EntityParams<'a> = (SceneId, u32, usize, &'a ParamValues);

pub(crate) fn entity_params(scene: &Scene, id: u32, slot: usize) -> Option<EntityParams<'_>> {
    scene
        .shader_overrides
        .of(id)
        .map(|own| (scene.id(), id, slot, own))
}

impl Renderer {
    pub(crate) fn material_index(
        &mut self,
        scene: SceneId,
        material: Option<(&str, &MaterialAsset)>,
        pipeline: usize,
        overrides: Option<EntityParams>,
    ) -> usize {
        let owner = material.and_then(|(key, asset)| {
            let layout = self.surface_shaders.params(pipeline)?;
            let (owner, values) = match overrides {
                Some((owner_scene, id, slot, own)) => {
                    let mut merged = asset.shader_params.clone();
                    merged.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
                    (
                        ParamOwner::Entity(owner_scene, id, slot),
                        layout.pack(&merged),
                    )
                }
                None => (
                    ParamOwner::Material(scene, key.to_owned()),
                    layout.pack(&asset.shader_params),
                ),
            };
            self.materials
                .write_params(&self.device, &self.queue, &owner, values);
            Some(owner)
        });
        let paths = material_texture_paths(material.map(|(_, asset)| asset));
        let key = (
            std::array::from_fn(|i| self.resolved_key(paths[i].as_ref())),
            owner,
        );
        if let Some(i) = self.materials.lookup(&key) {
            return i;
        }
        let maps = self.group_maps(&paths);
        let group = self.material_bind_group(&maps, key.1.as_ref());
        self.materials.insert(key, group, paths)
    }

    pub(crate) fn group_maps(&self, paths: &MapPaths) -> [Rc<GpuTexture>; MATERIAL_TEXTURES] {
        std::array::from_fn(|i| {
            let map = self.resolve_map(paths[i].as_ref(), map_kind(i));
            match i >= MATERIAL_MAPS && Rc::ptr_eq(&map, &self.default_texture) {
                true => Rc::clone(&self.white_texture),
                false => map,
            }
        })
    }

    fn resolve_map(&self, path: Option<&MapRef>, kind: TextureKind) -> Rc<GpuTexture> {
        path.and_then(|p| self.texture_for(p, kind))
            .unwrap_or_else(|| Rc::clone(&self.default_texture))
    }

    pub(crate) fn with_map_channels(
        &self,
        mut record: MaterialData,
        material: Option<&MaterialAsset>,
    ) -> MaterialData {
        let Some(m) = material else {
            return record;
        };
        // Flag 2: the shader reads red, where a one-channel upload keeps it (`maps.wgsl`'s
        // `map_factor`).
        let single = |map: &Option<MapRef>| {
            let texture = map.as_ref().and_then(|p| self.gpu_textures.get(p));
            texture.is_some_and(|t| t.single_channel)
        };
        if single(&m.metallic_map) {
            record.use_metallic_map = 2;
        }
        if single(&m.roughness_map) {
            record.use_roughness_map = 2;
        }
        if single(&m.specular_map) {
            record.level_maps.use_specular_map = 2;
        }
        let normal = m.normal_map.as_ref();
        let bound = normal.and_then(|p| self.texture_for(p, TextureKind::Normal));
        match bound.as_deref() {
            Some(t) if t.texture.format() == DEEP_FORMAT => {
                record.use_normal_map = SIGNED_NORMAL_MAP
            }
            Some(GpuTexture {
                normal_variance: Some(table),
                ..
            }) => {
                record.use_normal_map = 2;
                record.normal_variance = pack_variance(*table);
            }
            _ => {}
        }
        for slot in &MAP_SLOTS {
            let Some(flag) = slot.off_while_missing else {
                continue;
            };
            let path = (slot.path)(m);
            if path.and_then(|p| self.texture_for(p, slot.kind)).is_none() {
                *flag(&mut record) = 0;
            }
        }
        record
    }

    fn resolved_key(&self, path: Option<&MapRef>) -> String {
        match path {
            Some(p) if self.gpu_textures.contains_key(p) => self
                .render_texture_key(p.as_str())
                .unwrap_or_else(|| p.to_string()),
            _ => String::new(),
        }
    }

    pub(crate) fn material_bind_group(
        &self,
        maps: &[Rc<GpuTexture>; MATERIAL_TEXTURES],
        owner: Option<&ParamOwner>,
    ) -> wgpu::BindGroup {
        let maps = maps.each_ref().map(|m| m.as_ref());
        let params = self.materials.params_buffer(owner);
        material_group(&self.device, &self.material_layout, maps, params)
    }

    #[cfg(test)]
    pub(crate) fn material_group_count(&self) -> usize {
        self.materials.len()
    }
}

pub(crate) fn material_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    maps: [&GpuTexture; MATERIAL_TEXTURES],
    params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let mut entries = Vec::with_capacity(2 * MATERIAL_TEXTURES + 1);
    for (i, (map, binding)) in maps.into_iter().zip(all_bindings()).enumerate() {
        let view = match map_kind(i) == TextureKind::Colour {
            true => &map.view,
            false => &map.data_view,
        };
        entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: map_sampler_binding(binding),
            resource: wgpu::BindingResource::Sampler(&map.sampler),
        });
    }
    entries.push(wgpu::BindGroupEntry {
        binding: rusty_shadergen::params::PARAM_BINDING,
        resource: params.as_entire_binding(),
    });
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Material Bind Group"),
        layout,
        entries: &entries,
    })
}

fn pack_variance(table: rusty_asset::texture_import::variance::VarianceTable) -> [u32; 4] {
    std::array::from_fn(|w| u32::from_le_bytes(std::array::from_fn(|b| table[w * 4 + b])))
}
