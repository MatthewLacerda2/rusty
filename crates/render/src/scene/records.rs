use std::collections::{BTreeMap, BTreeSet};

use super::classes::Classes;
use super::inputs::Library;
use super::table::{SlotTable, Slotted};
use crate::draw::batch::RecordTraits;
use crate::draw::uniforms::material_data;
use crate::passes::shadows::Cutout;
use crate::MaterialData;
use rusty_components::MaterialAsset;
use rusty_scene::SceneId;

impl Slotted for MaterialData {
    fn identity() -> Self {
        MaterialData::flat([1.0; 4])
    }
}

/// Lit: what a mesh entity draws; Unlit: a light's own mesh; Debris: a mesh particle (no decals).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Variant {
    Lit,
    Unlit,
    Debris,
}

impl Variant {
    const ALL: [Self; 3] = [Self::Lit, Self::Unlit, Self::Debris];

    fn at(self) -> usize {
        self as usize
    }

    pub(crate) fn record(self, material: Option<&MaterialAsset>) -> MaterialData {
        let record = material_data(self != Self::Unlit, material);
        match self {
            Self::Debris => MaterialData {
                receive_decals: 0,
                ..record
            },
            _ => record,
        }
    }
}

type Slots = [Option<u32>; 3];

#[derive(Default)]
pub(crate) struct MaterialRecords {
    table: SlotTable<MaterialData>,
    classes: Classes,
    named: BTreeMap<SceneId, BTreeMap<String, Slots>>,
    unnamed: BTreeMap<SceneId, Slots>,
}

impl MaterialRecords {
    pub(crate) fn slot(
        &mut self,
        (scene, name, variant): (SceneId, Option<&str>, Variant),
        data: MaterialData,
    ) -> u32 {
        let slots = match name {
            Some(name) => {
                let names = self.named.entry(scene).or_default();
                if !names.contains_key(name) {
                    names.insert(name.to_owned(), [None; 3]);
                }
                names.get_mut(name).expect("inserted above")
            }
            None => self.unnamed.entry(scene).or_default(),
        };
        let table = &mut self.table;
        let slot = *slots[variant.at()].get_or_insert_with(|| table.alloc());
        self.write(slot, data);
        slot
    }

    pub(crate) fn write(&mut self, slot: u32, data: MaterialData) {
        self.table.write(slot, data);
        self.classes.set(slot, &data);
    }

    pub(crate) fn rep(&self, slot: u32) -> u32 {
        self.classes.rep(slot)
    }

    pub(crate) fn table(&self) -> &SlotTable<MaterialData> {
        &self.table
    }

    pub(crate) fn table_for_sync(&mut self) -> &mut SlotTable<MaterialData> {
        &mut self.table
    }

    fn free(&mut self, slot: u32) {
        self.classes.clear(slot);
        self.table.release(slot);
    }

    pub(crate) fn variants_of(&self, scene: SceneId, name: &str) -> Vec<(Variant, u32)> {
        let slots = self.named.get(&scene).and_then(|n| n.get(name));
        let slots = slots.copied().unwrap_or_default();
        let at = Variant::ALL.into_iter().zip(slots);
        at.filter_map(|(v, s)| Some((v, s?))).collect()
    }

    pub(crate) fn release_name(&mut self, scene: SceneId, name: &str) {
        let slots = self.named.get_mut(&scene).and_then(|n| n.remove(name));
        for slot in slots.into_iter().flatten().flatten() {
            self.free(slot);
        }
    }

    pub(crate) fn release_scene(&mut self, scene: SceneId) {
        let named = self
            .named
            .remove(&scene)
            .into_iter()
            .flat_map(|n| n.into_values());
        let slots: Vec<u32> = named
            .chain(self.unnamed.remove(&scene))
            .flatten()
            .flatten()
            .collect();
        for slot in slots {
            self.free(slot);
        }
    }

    #[cfg(test)]
    pub(crate) fn classes_hold(&self) -> Result<(), String> {
        let slots = self.kept();
        let bytes = |s: u32| bytemuck::bytes_of(self.table.get(s)).to_vec();
        let least = |s| {
            slots
                .iter()
                .copied()
                .filter(|&o| bytes(o) == bytes(s))
                .min()
        };
        match slots.iter().find(|&&s| Some(self.rep(s)) != least(s)) {
            Some(&s) => Err(format!("slot {s} names {} not {:?}", self.rep(s), least(s))),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    pub(crate) fn kept(&self) -> Vec<u32> {
        let named = self.named.values().flat_map(|n| n.values());
        let all = named.chain(self.unnamed.values()).flatten().flatten();
        all.copied().collect()
    }
}

/// Record-only fields are set aside, but what they feed on the CPU (`RecordTraits`, `Cutout`) still
/// compares, so a value that flips a pass re-keys.
pub(crate) fn rekeys(old: &MaterialAsset, new: &MaterialAsset) -> bool {
    let traits = |m| Variant::ALL.map(|v| RecordTraits::of(&v.record(Some(m))));
    record_free(old) != record_free(new)
        || traits(old) != traits(new)
        || Cutout::of(old) != Cutout::of(new)
}

pub(crate) fn params_moved(old: &MaterialAsset, new: &MaterialAsset) -> bool {
    old.shader_params != new.shader_params
}

fn record_free(m: &MaterialAsset) -> MaterialAsset {
    let d = MaterialAsset::default();
    MaterialAsset {
        base_color: d.base_color,
        metallic: d.metallic,
        roughness: d.roughness,
        emissive: d.emissive,
        occlusion_strength: d.occlusion_strength,
        height_scale: d.height_scale,
        tiling: d.tiling,
        offset: d.offset,
        uv_mapping: d.uv_mapping,
        world_size: d.world_size,
        blend_sharpness: d.blend_sharpness,
        detail: m
            .detail
            .as_ref()
            .and_then(rusty_components::DetailLayer::map_only),
        alpha: d.alpha,
        alpha_cutoff: d.alpha_cutoff,
        blend_mode: d.blend_mode,
        preserve_specular: d.preserve_specular,
        receive_shadows: d.receive_shadows,
        refraction_model: d.refraction_model,
        ior: d.ior,
        specular: d.specular,
        specular_color: d.specular_color,
        thickness: d.thickness,
        transmittance_color: d.transmittance_color,
        absorption_distance: d.absorption_distance,
        depth_postpass: d.depth_postpass,
        receive_ssr: d.receive_ssr,
        reactive: d.reactive,
        transmission_color: d.transmission_color,
        transmission: d.transmission,
        receive_decals: d.receive_decals,
        shader_params: d.shader_params,
        ..m.clone()
    }
}

#[cfg(test)]
#[path = "records_tests.rs"]
mod tests;

pub(super) fn library_diff(old: &Library, new: &Library) -> BTreeSet<String> {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let (mut a, mut b) = (old.iter().peekable(), new.iter().peekable());
    let mut edited = BTreeSet::new();
    loop {
        let name = match (a.peek(), b.peek()) {
            (None, None) => return edited,
            (Some(_), None) => a.next().map(|(n, _)| n),
            (None, Some(_)) => b.next().map(|(n, _)| n),
            (Some((x, ma)), Some((y, mb))) => match x.cmp(y) {
                Less => a.next().map(|(n, _)| n),
                Greater => b.next().map(|(n, _)| n),
                Equal if ma == mb => {
                    a.next();
                    b.next();
                    None
                }
                Equal => {
                    b.next();
                    a.next().map(|(n, _)| n)
                }
            },
        };
        edited.extend(name.cloned());
    }
}
