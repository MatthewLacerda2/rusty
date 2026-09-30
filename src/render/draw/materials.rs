//! src/render/draw/materials.rs — the material bind-group cache: a material's five
//! maps resolved to resident textures and folded into one shared group-2 bind group
//! per distinct signature (#202, #207, #470). Used by the entity solids and by mesh
//! particles (#440), so the same material always binds the same group.

use std::rc::Rc;

use crate::components::MaterialAsset;
use crate::render::{GpuTexture, Renderer};

impl Renderer {
    /// The material cache index for `material`'s five maps (albedo, metallic,
    /// roughness, normal, emissive), building the bind group on first use. The key is
    /// the *resolved* signature, so a late-loaded texture gets a fresh group (#207).
    pub(crate) fn material_index(&mut self, material: Option<&MaterialAsset>) -> usize {
        let paths = [
            material.and_then(|m| m.base_color_map.clone()),
            material.and_then(|m| m.metallic_map.clone()),
            material.and_then(|m| m.roughness_map.clone()),
            material.and_then(|m| m.normal_map.clone()),
            material.and_then(|m| m.emissive_map.clone()),
        ];
        let sig = std::array::from_fn(|i| self.resolved_key(paths[i].as_ref()));
        if let Some(i) = self.materials.lookup(&sig) {
            return i;
        }
        let maps = std::array::from_fn(|i| self.resolve_map(paths[i].as_ref()));
        let group = self.material_bind_group(&maps);
        self.materials.insert(sig, group)
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
    /// else empty (the default texture). Lets the pool detect a late-loaded map.
    fn resolved_key(&self, path: Option<&String>) -> String {
        match path {
            Some(p) if self.gpu_textures.contains_key(p) => p.clone(),
            _ => String::new(),
        }
    }

    /// Build a group(2) material bind group from the five resolved map textures
    /// (albedo, metallic, roughness, normal, emissive — that order) + one shared
    /// sampler, against `material_layout`. Textures bind at 0,2,3,4,5; sampler at 1
    /// (binding 1 samples all five) (#202, #207).
    pub(crate) fn material_bind_group(&self, maps: &[Rc<GpuTexture>; 5]) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&self.default_texture.sampler),
        }];
        for (map, binding) in maps.iter().zip([0u32, 2, 3, 4, 5]) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(&map.view),
            });
        }
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
