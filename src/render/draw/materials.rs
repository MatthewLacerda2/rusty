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

use crate::asset::MapRef;
use crate::components::MaterialAsset;
use crate::render::gpu::material_cache::{ParamOwner, MATERIAL_TEXTURES};
use crate::render::gpu::textures::compressed::is_single_channel;
use crate::render::gpu::textures::TextureKind;
use crate::render::{EntityUniform, GpuTexture, Renderer};
use crate::scene::shader_overrides::ParamValues;
use crate::scene::{Scene, SceneId};
use crate::shadergen::textures;

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
    /// The material cache index for `material` — `(library key, asset)` — drawn with
    /// pipeline `pipeline`: its five maps (albedo, metallic, roughness, normal,
    /// emissive), building the bind group on first use. The key is the *resolved*
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
        material: Option<(&str, &MaterialAsset)>,
        pipeline: usize,
        overrides: Option<EntityParams>,
    ) -> usize {
        let owner = material.and_then(|(key, asset)| {
            let layout = self.surface_shaders.params(pipeline)?;
            let (owner, values) = match overrides {
                Some((scene, id, slot, own)) => {
                    let mut merged = asset.shader_params.clone();
                    merged.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
                    (ParamOwner::Entity(scene, id, slot), layout.pack(&merged))
                }
                None => (
                    ParamOwner::Material(key.to_owned()),
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
            let map = self.resolve_map(paths[i].as_ref());
            // An extra slot with no texture of its own is a neutral white mask,
            // not the checker a missing map shows (#400).
            match i >= 5 && Rc::ptr_eq(&map, &self.default_texture) {
                true => Rc::clone(&self.white_texture),
                false => map,
            }
        });
        let group = self.material_bind_group(&maps, key.1.as_ref());
        self.materials.insert(key, group)
    }

    /// Resolve a material map path to a resident GPU texture, falling back to the
    /// default texture when the path is absent or not yet uploaded.
    fn resolve_map(&self, path: Option<&MapRef>) -> Rc<GpuTexture> {
        match path {
            Some(p) => self
                .gpu_textures
                .get(p)
                .cloned()
                .unwrap_or_else(|| Rc::clone(&self.default_texture)),
            None => Rc::clone(&self.default_texture),
        }
    }

    /// `uniform` with its metallic and roughness flags set to 2 where the map is
    /// resident as one channel (BC4, #1574): the shader then reads its red channel
    /// instead of glTF's packed blue and green.
    pub(crate) fn with_map_channels(
        &self,
        mut uniform: EntityUniform,
        material: Option<&MaterialAsset>,
    ) -> EntityUniform {
        let Some(m) = material else {
            return uniform;
        };
        let single = |map: &Option<MapRef>| {
            let texture = map.as_ref().and_then(|p| self.gpu_textures.get(p));
            texture.is_some_and(|t| is_single_channel(&t.texture))
        };
        if single(&m.metallic_map) {
            uniform.use_metallic_map = 2;
        }
        if single(&m.roughness_map) {
            uniform.use_roughness_map = 2;
        }
        uniform
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

    /// Build a group(2) material bind group from the resolved textures (albedo,
    /// metallic, roughness, normal, emissive, then the extra slots — that order),
    /// against `material_layout`. Maps bind at 0,2,3,4,5 (#202, #207), each extra
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

/// A group(2) bind group against `layout` over `maps` (in
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
    let slots = textures::SLOTS.iter().map(|slot| slot.binding);
    let bindings = [0u32, 2, 3, 4, 5].into_iter().chain(slots);
    for (i, (map, binding)) in maps.into_iter().zip(bindings).enumerate() {
        let view = match is_colour_map(i) {
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
        binding: crate::shadergen::params::PARAM_BINDING,
        resource: params.as_entire_binding(),
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Material Bind Group"),
        layout,
        entries: &entries,
    })
}

/// The binding of the sampler paired with the material texture at `binding`:
/// albedo's `s_diffuse` at 1, every other at `textures::sampler_binding` (#1323).
fn map_sampler_binding(binding: u32) -> u32 {
    match binding {
        0 => 1,
        b => textures::sampler_binding(b),
    }
}

/// Whether the map at `index` (in [`material_texture_paths`] order) holds colour —
/// albedo or emissive, sampled through the sRGB-decoding view. Every other map and
/// shader texture slot holds data and samples its texels raw (glTF 2.0, #647).
pub(crate) fn is_colour_map(index: usize) -> bool {
    matches!(index, 0 | 4)
}

/// Every texture `material` samples (as [`material_texture_paths`]), each with what
/// its slot holds, which decides how its mips average (#1322): what to upload
/// before its bind group is built.
pub(crate) fn material_maps(
    material: Option<&MaterialAsset>,
) -> impl Iterator<Item = (MapRef, TextureKind)> {
    material_texture_paths(material)
        .into_iter()
        .enumerate()
        .filter_map(|(i, path)| Some((path?, map_kind(i))))
}

/// What the map at `index` holds, which decides how its mips average (#1322) and
/// what it compresses to (#1574): metallic and roughness are masks, the normal map
/// a normal map, the shader texture slots data.
fn map_kind(index: usize) -> TextureKind {
    match index {
        1 | 2 => TextureKind::Mask,
        3 => TextureKind::Normal,
        i if is_colour_map(i) => TextureKind::Colour,
        _ => TextureKind::Data,
    }
}

/// Every texture path `material`'s group 2 binds, in [`MapSignature`] order: the
/// five maps, then the texture each extra shader slot names (#400). Also the list
/// of textures to upload before the group is built.
///
/// [`MapSignature`]: crate::render::gpu::material_cache::MapSignature
pub(crate) fn material_texture_paths(
    material: Option<&MaterialAsset>,
) -> [Option<MapRef>; MATERIAL_TEXTURES] {
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
        .map(|s| m.shader_textures.get(s.name));
    let mut paths = maps.into_iter().map(Option::as_ref).chain(slots);
    std::array::from_fn(|_| paths.next().flatten().cloned())
}
