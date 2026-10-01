//! src/render/draw/materials.rs — the material bind-group cache: a material's five
//! maps resolved to resident textures and folded into one shared group-2 bind group
//! per distinct signature (#202, #207, #470). Used by the entity solids and by mesh
//! particles (#440), so the same material always binds the same group. A material
//! whose surface shader has runtime params (#399) gets its own group over its own
//! param buffer, rewritten here when its values change. The extra shader texture
//! slots (`mask`, #400) bind after the maps, white when a material names none.
//! Albedo and emissive bind a texture's sRGB-decoding view; the data maps bind its
//! raw view, so one upload serves a file named in both roles (#647).

use std::rc::Rc;

use crate::components::MaterialAsset;
use crate::render::gpu::material_cache::MATERIAL_TEXTURES;
use crate::render::{GpuTexture, Renderer};
use crate::shadergen::textures;

impl Renderer {
    /// The material cache index for `material` — `(library key, asset)` — drawn with
    /// pipeline `pipeline`: its five maps (albedo, metallic, roughness, normal,
    /// emissive), building the bind group on first use. The key is the *resolved*
    /// signature, so a late-loaded texture gets a fresh group (#207). When the
    /// pipeline's shader has runtime params, the material's values are packed into
    /// its own param buffer first (#399).
    pub(crate) fn material_index(
        &mut self,
        material: Option<(&str, &MaterialAsset)>,
        pipeline: usize,
    ) -> usize {
        let owner = material.and_then(|(key, asset)| {
            let layout = self.surface_shaders.params(pipeline)?;
            let values = layout.pack(&asset.shader_params);
            self.materials
                .write_params(&self.device, &self.queue, key, values);
            Some(key.to_owned())
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
            let map = self.resolve_map(paths[i].as_ref());
            // An extra slot with no texture of its own is a neutral white mask,
            // not the checker a missing map shows (#400).
            match i >= 5 && Rc::ptr_eq(&map, &self.default_texture) {
                true => Rc::clone(&self.white_texture),
                false => map,
            }
        });
        let group = self.material_bind_group(&maps, key.1.as_deref());
        self.materials.insert(key, group)
    }

    /// Resolve a material map path to a resident GPU texture, falling back to the
    /// default texture when the path is absent or not yet uploaded.
    fn resolve_map(&self, path: Option<&String>) -> Rc<GpuTexture> {
        match path {
            Some(p) => self
                .gpu_textures
                .get(p)
                .cloned()
                .unwrap_or_else(|| Rc::clone(&self.default_texture)),
            None => Rc::clone(&self.default_texture),
        }
    }

    /// The cache key a map path resolves to: the path when its texture is resident,
    /// else empty (the default texture). Lets the pool detect a late-loaded map. A
    /// render texture's key also names its allocation (#430), so a resize rebinds.
    fn resolved_key(&self, path: Option<&String>) -> String {
        match path {
            Some(p) if self.gpu_textures.contains_key(p) => {
                self.render_texture_key(p).unwrap_or_else(|| p.clone())
            }
            _ => String::new(),
        }
    }

    /// Build a group(2) material bind group from the resolved textures (albedo,
    /// metallic, roughness, normal, emissive, then the extra slots — that order) +
    /// one shared sampler, against `material_layout`. Maps bind at 0,2,3,4,5; sampler
    /// at 1 (binding 1 samples them all) (#202, #207); `owner`'s param buffer (or the
    /// shared zero one) at 6 (#399); each extra slot at its own binding (7, #400).
    pub(crate) fn material_bind_group(
        &self,
        maps: &[Rc<GpuTexture>; MATERIAL_TEXTURES],
        owner: Option<&str>,
    ) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&self.default_texture.sampler),
        }];
        let slots = textures::SLOTS.iter().map(|slot| slot.binding);
        let bindings = [0u32, 2, 3, 4, 5].into_iter().chain(slots);
        for (i, (map, binding)) in maps.iter().zip(bindings).enumerate() {
            let view = match is_colour_map(i) {
                true => &map.view,
                false => &map.data_view,
            };
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: crate::shadergen::params::PARAM_BINDING,
            resource: self.materials.params_buffer(owner).as_entire_binding(),
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &self.material_layout,
            entries: &entries,
        })
    }

    /// Distinct material bind groups built so far — tests prove they are shared and
    /// do not grow per frame.
    #[cfg(test)]
    pub(crate) fn material_group_count(&self) -> usize {
        self.materials.len()
    }
}

/// Whether the map at `index` (in [`material_texture_paths`] order) holds colour —
/// albedo or emissive, sampled through the sRGB-decoding view. Every other map and
/// shader texture slot holds data and samples its texels raw (glTF 2.0, #647).
pub(crate) fn is_colour_map(index: usize) -> bool {
    matches!(index, 0 | 4)
}

/// Every texture path `material`'s group 2 binds, in [`MapSignature`] order: the
/// five maps, then the texture each extra shader slot names (#400). Also the list
/// of textures to upload before the group is built.
///
/// [`MapSignature`]: crate::render::gpu::material_cache::MapSignature
pub(crate) fn material_texture_paths(
    material: Option<&MaterialAsset>,
) -> [Option<String>; MATERIAL_TEXTURES] {
    let Some(m) = material else {
        return Default::default();
    };
    let maps = [
        &m.base_color_map,
        &m.metallic_map,
        &m.roughness_map,
        &m.normal_map,
        &m.emissive_map,
    ];
    let slots = textures::SLOTS
        .iter()
        .map(|s| m.shader_textures.get(s.name).cloned());
    let mut paths = maps.into_iter().cloned().chain(slots);
    std::array::from_fn(|_| paths.next().flatten())
}
