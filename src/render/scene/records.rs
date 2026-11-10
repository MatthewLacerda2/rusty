//! src/render/scene/records.rs — the renderer's material table (#1706): one
//! [`MaterialData`] per (scene, library material, [`Variant`]), at a slot of a
//! [`SlotTable`] mirrored on the GPU (group 1 binding 4), which a draw's uniform names.
//!
//! **Why a variant.** A record is the material's, but two inputs of it are not:
//! a light's own mesh draws its material unlit (`is_lit`, and the decal, preserve
//! specular, refraction and SSR terms that follow it), and a mesh particle never
//! takes decals. Each is a fixed transform of the material, so each is its own record
//! of it, and an edit rewrites every variant drawn. Nothing else a record holds reads
//! the entity: a slot's per-object overrides live in its param buffer (group 2,
//! `ParamOwner::Entity`), the world matrix, probe SH and lightmap in its object record.
//!
//! **Lifetime.** A slot is allocated the first time a draw names its record (an
//! object's derive, a mesh emitter's key) and kept while its scene is: freed when its
//! name leaves the library ([`MaterialRecords::release_name`]) or its scene is evicted
//! from the retained set ([`MaterialRecords::release_scene`]). Slot 0 holds the flat
//! white record, which a shadow caster that reads none binds.
//!
//! **An edit.** The extract sorts each edited library name by [`rekeys`]: an edit that
//! moves nothing a key, a clip or the static bakes read (a colour, a scalar) rewrites
//! the name's records in place and re-derives no object; any other edit re-derives
//! its users, as before.
//!
//! **Batching.** Equal records under different names (each entity's own
//! `Material.Set*` material) are one value class (`classes.rs`): a frame names a
//! slot by [`MaterialRecords::rep`] before it batches, so they share a draw.

use std::collections::{BTreeMap, BTreeSet};

use super::classes::Classes;
use super::inputs::Library;
use super::table::{SlotTable, Slotted};
use crate::components::MaterialAsset;
use crate::render::draw::batch::RecordTraits;
use crate::render::draw::uniforms::material_data;
use crate::render::passes::shadows::Cutout;
use crate::render::MaterialData;
use crate::scene::SceneId;

impl Slotted for MaterialData {
    fn identity() -> Self {
        MaterialData::flat([1.0; 4])
    }
}

/// Which transform of its material a record holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Variant {
    /// What a mesh entity draws.
    Lit,
    /// What a light's own mesh draws: unlit.
    Unlit,
    /// What a mesh particle draws: lit, taking no decals (#638).
    Debris,
}

impl Variant {
    const ALL: [Self; 3] = [Self::Lit, Self::Unlit, Self::Debris];

    fn at(self) -> usize {
        self as usize
    }

    /// The record of `material` under this variant, before its map channels
    /// (`Renderer::with_map_channels`) are set.
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

/// One name's (or no material's) slots, by [`Variant`].
type Slots = [Option<u32>; 3];

/// Every kept record's slot, by scene, name and variant, the table they sit in, and
/// the slots' value classes (`classes.rs`): every write goes through here, so a class
/// never lags its slot.
#[derive(Default)]
pub(crate) struct MaterialRecords {
    table: SlotTable<MaterialData>,
    classes: Classes,
    named: BTreeMap<SceneId, BTreeMap<String, Slots>>,
    /// The record a draw naming no library material reads, per scene.
    unnamed: BTreeMap<SceneId, Slots>,
}

impl MaterialRecords {
    /// The slot of scene `scene`'s record of `name` (`None`: no material) under
    /// `variant`, allocated when new, made to hold `data`.
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

    /// Make kept slot `slot` hold `data` (a record-only edit's rewrite).
    pub(crate) fn write(&mut self, slot: u32, data: MaterialData) {
        self.table.write(slot, data);
        self.classes.set(slot, &data);
    }

    /// The slot a draw naming `slot` names, so equal records batch: the least slot
    /// holding the same bytes (`classes.rs`).
    pub(crate) fn rep(&self, slot: u32) -> u32 {
        self.classes.rep(slot)
    }

    /// The table, as the GPU mirror and the shadow sweeps read it.
    pub(crate) fn table(&self) -> &SlotTable<MaterialData> {
        &self.table
    }

    /// The table, for the GPU mirror to take its dirty runs: no record is written
    /// through this (that is [`Self::write`]'s, which keeps the classes).
    pub(crate) fn table_for_sync(&mut self) -> &mut SlotTable<MaterialData> {
        &mut self.table
    }

    fn free(&mut self, slot: u32) {
        self.classes.clear(slot);
        self.table.release(slot);
    }

    /// The variants of `name` scene `scene` keeps, with their slots.
    pub(crate) fn variants_of(&self, scene: SceneId, name: &str) -> Vec<(Variant, u32)> {
        let slots = self.named.get(&scene).and_then(|n| n.get(name));
        let slots = slots.copied().unwrap_or_default();
        let at = Variant::ALL.into_iter().zip(slots);
        at.filter_map(|(v, s)| Some((v, s?))).collect()
    }

    /// `name` left scene `scene`'s library: its records' slots are freed.
    pub(crate) fn release_name(&mut self, scene: SceneId, name: &str) {
        let slots = self.named.get_mut(&scene).and_then(|n| n.remove(name));
        for slot in slots.into_iter().flatten().flatten() {
            self.free(slot);
        }
    }

    /// Scene `scene` was evicted from the retained set: every record of it is freed.
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

    /// Every kept record names, as its class's representative, the least kept slot
    /// holding its very bytes.
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

    /// Every slot a record holds, across the scenes.
    #[cfg(test)]
    pub(crate) fn kept(&self) -> Vec<u32> {
        let named = self.named.values().flat_map(|n| n.values());
        let all = named.chain(self.unnamed.values()).flatten().flatten();
        all.copied().collect()
    }
}

/// Whether an edit from `old` to `new` moves anything a draw's key, its shadow clip or
/// the static bakes read: then its users are re-derived. When it doesn't, only the
/// record's values (and the param buffer's) moved, and the records are rewritten in
/// place. The fields a record alone reads are set aside and the rest compared whole,
/// then what the record's values feed on the CPU ([`RecordTraits`] under every
/// variant, the Cutout shadow test) is compared too, so a scalar that flips a pass
/// (a roughness that stops a glass tracing SSR) still re-keys.
pub(crate) fn rekeys(old: &MaterialAsset, new: &MaterialAsset) -> bool {
    let traits = |m| Variant::ALL.map(|v| RecordTraits::of(&v.record(Some(m))));
    record_free(old) != record_free(new)
        || traits(old) != traits(new)
        || Cutout::of(old) != Cutout::of(new)
}

/// Whether `old` and `new` differ in their shader params.
pub(crate) fn params_moved(old: &MaterialAsset, new: &MaterialAsset) -> bool {
    old.shader_params != new.shader_params
}

/// `m` with every field only its record and param buffer read set to the default.
fn record_free(m: &MaterialAsset) -> MaterialAsset {
    let d = MaterialAsset::default();
    MaterialAsset {
        base_color: d.base_color,
        metallic: d.metallic,
        roughness: d.roughness,
        emissive: d.emissive,
        tiling: d.tiling,
        offset: d.offset,
        uv_mapping: d.uv_mapping,
        world_size: d.world_size,
        blend_sharpness: d.blend_sharpness,
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

/// The names added, removed or edited between `old` and `new`: one merge walk of
/// the two sorted maps, each name visited once.
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
