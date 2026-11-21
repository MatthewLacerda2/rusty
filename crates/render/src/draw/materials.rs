//! crates/render/src/draw/materials.rs — the material bind-group cache: a material's eight
//! maps (`map_slots::MAP_SLOTS`, #1997, #2151) resolved to resident textures and
//! folded into one shared material bind group (`groups::MATERIAL`) per distinct signature (#202, #207, #470). Used by the entity solids and by mesh
//! particles (#440), so the same material always binds the same group. A material
//! whose surface shader has runtime params (#399) gets its own group over its own
//! param buffer, rewritten here when its values change. The extra shader texture
//! slots (`mask`, #400) bind after the maps, white when a material names none.
//! Albedo and emissive bind a texture's sRGB-decoding view; the data maps bind its
//! raw view, so one upload serves a file named in both roles (#647).

use crate::gpu::ops::CountedDevice as _;
use std::rc::Rc;

use super::map_slots::{
    all_bindings, map_kind, map_sampler_binding, material_texture_paths, MAP_SLOTS,
};
use crate::gpu::material_cache::{ParamOwner, MATERIAL_MAPS, MATERIAL_TEXTURES};
use crate::gpu::textures::TextureKind;
use crate::{GpuTexture, MaterialData, Renderer};
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_scene::shader_overrides::ParamValues;
use rusty_scene::{Scene, SceneId};

/// One drawn entity's own param values (#670): its scene, its id, the material slot
/// drawn (#1285), its overrides.
pub(crate) type EntityParams<'a> = (SceneId, u32, usize, &'a ParamValues);

/// Entity `id`'s overrides in `scene`, for its material slot `slot`, in the form
/// [`Renderer::material_index`] takes.
pub(crate) fn entity_params(scene: &Scene, id: u32, slot: usize) -> Option<EntityParams<'_>> {
    scene
        .shader_overrides
        .of(id)
        .map(|own| (scene.id(), id, slot, own))
}

impl Renderer {
    /// The material cache index for `material` — `(library key, asset)` of scene
    /// `scene`'s library — drawn with pipeline `pipeline`: its ten maps (albedo, metallic, roughness, normal,
    /// emissive, detail, occlusion, specular, height, detail mask), building the bind group on first use. The key is the *resolved*
    /// signature, so a late-loaded texture gets a fresh group (#207). When the
    /// pipeline's shader has runtime params, the material's values are packed into
    /// its own param buffer first (#399).
    ///
    /// `overrides` is the drawn entity's own values (#670): when it has any and the
    /// shader has runtime params, the entity owns a buffer per material slot (#1285)
    /// holding that slot's material values with its overrides on top, and a group of
    /// its own — so two slots with different materials never share, or rewrite each
    /// frame, one buffer.
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
        let maps = std::array::from_fn(|i| {
            let map = self.resolve_map(paths[i].as_ref(), map_kind(i));
            // An extra slot with no texture of its own is a neutral white mask,
            // not the checker a missing map shows (#400).
            match i >= MATERIAL_MAPS && Rc::ptr_eq(&map, &self.default_texture) {
                true => Rc::clone(&self.white_texture),
                false => map,
            }
        });
        let group = self.material_bind_group(&maps, key.1.as_ref());
        self.materials.insert(key, group)
    }

    /// Resolve a material map path to a resident GPU texture for a slot holding
    /// `kind` (its uncompressed copy where the slot cannot read the upload's
    /// format, #1574), falling back to the default texture when the path is
    /// absent or not yet uploaded.
    fn resolve_map(&self, path: Option<&MapRef>, kind: TextureKind) -> Rc<GpuTexture> {
        path.and_then(|p| self.texture_for(p, kind))
            .unwrap_or_else(|| Rc::clone(&self.default_texture))
    }

    /// `record` with its metallic and roughness flags set to 2 where the map is
    /// resident and of Texture Type `SingleChannel` (#1574), BC4 or not yet: the
    /// shader then reads its red channel instead of glTF's packed blue and green,
    /// so the look does not change when the compressed copy lands. Its normal flag
    /// is 2 where the bound normal map is BC5, its table in the record (#1702), and
    /// its specular flag 2 where that map is `SingleChannel` (red, not glTF's alpha,
    /// #2151). Every map whose row switches off while missing
    /// ([`MapSlot::off_while_missing`]: detail, occlusion, specular level, height, detail mask) has its
    /// flag 0 while its texture is not resident.
    ///
    /// [`MapSlot::off_while_missing`]: super::map_slots::MapSlot::off_while_missing
    pub(crate) fn with_map_channels(
        &self,
        mut record: MaterialData,
        material: Option<&MaterialAsset>,
    ) -> MaterialData {
        let Some(m) = material else {
            return record;
        };
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
        if let Some(table) = bound.and_then(|t| t.normal_variance) {
            record.use_normal_map = 2;
            record.normal_variance = pack_variance(table);
        }
        // A map that darkens or scales and is not resident (missing, loading, an
        // empty path) draws as unmapped, never over the checker its slot binds
        // meanwhile (#1997, #2151).
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

    /// The cache key a map path resolves to: the path when its texture is resident,
    /// else empty (the default texture). Lets the pool detect a late-loaded map. A
    /// render texture's key also names its allocation (#430), so a resize rebinds.
    fn resolved_key(&self, path: Option<&MapRef>) -> String {
        match path {
            Some(p) if self.gpu_textures.contains_key(p) => self
                .render_texture_key(p.as_str())
                .unwrap_or_else(|| p.to_string()),
            _ => String::new(),
        }
    }

    /// Build a material bind group (`groups::MATERIAL`) from the resolved textures (the ten
    /// maps in `MAP_SLOTS` order: albedo, metallic, roughness, normal, emissive,
    /// detail, occlusion, specular level, height, detail mask; then the extra slots), against
    /// `material_layout`. Each map binds at its row's binding (#202, #207, #1997,
    /// #2151, #2191), each extra
    /// slot at its own binding (7, #400), and each texture's own sampler — its
    /// import settings' wrap and filter (#1323) — beside it: albedo's at 1, the
    /// rest at `textures::sampler_binding`. `owner`'s param buffer (or the shared
    /// zero one) binds at 6 (#399).
    pub(crate) fn material_bind_group(
        &self,
        maps: &[Rc<GpuTexture>; MATERIAL_TEXTURES],
        owner: Option<&ParamOwner>,
    ) -> wgpu::BindGroup {
        let maps = maps.each_ref().map(|m| m.as_ref());
        let params = self.materials.params_buffer(owner);
        material_group(&self.device, &self.material_layout, maps, params)
    }

    /// Distinct material bind groups built so far — tests prove they are shared and
    /// do not grow per frame.
    #[cfg(test)]
    pub(crate) fn material_group_count(&self) -> usize {
        self.materials.len()
    }
}

/// A material bind group against `layout` over `maps` (in
/// [`material_texture_paths`] order) and the param uniform `params`: the layout
/// [`Renderer::material_bind_group`] describes. The one builder of a material
/// group, so the default group and every material's bind the same entries.
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

/// A [`VarianceTable`](rusty_asset::texture_import::variance::VarianceTable) as
/// the record's four words, low byte first (`specular_aa::table_entry`).
fn pack_variance(table: rusty_asset::texture_import::variance::VarianceTable) -> [u32; 4] {
    std::array::from_fn(|w| u32::from_le_bytes(std::array::from_fn(|b| table[w * 4 + b])))
}
