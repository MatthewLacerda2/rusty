use std::collections::HashSet;

use crate::passes::shadows::Cutout;
use rusty_asset::MapRef;
use rusty_components::{MeshComponent, RenderFace, ShadowCasting};
use rusty_scene::{Scene, SceneId};

pub(crate) type StaticsKey = (SceneId, u64, u64);

#[cfg(test)]
pub(crate) fn statics_key(scene: &Scene) -> StaticsKey {
    let world = &scene.world;
    let unbaked = UnbakedCasters::of(scene);
    let statics = world.ids_with_mesh().into_iter().filter_map(|id| {
        let mesh = world.mesh(id)?;
        let casting = world
            .is_static(id)
            .then(|| ObjectCasting::of(scene, id, &mesh));
        Some((id, casting))
    });
    let statics: Vec<_> = statics.collect();
    let refs = statics
        .iter()
        .filter_map(|(id, c)| Some((*id, c.as_ref()?)));
    let casting = casting_key(refs, &unbaked);
    (scene.id(), world.static_revision(), casting)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UnbakedCasters {
    grouped: HashSet<u32>,
}

impl UnbakedCasters {
    pub(crate) fn of(scene: &Scene) -> Self {
        let world = &scene.world;
        let mut grouped = HashSet::new();
        for id in world.ids_with_lod_group() {
            let Some(group) = world.lod_group(id) else {
                continue;
            };
            let culls = |r: u32| world.mesh(r).is_some_and(|m| m.culls_when_small());
            if group.renderers().any(culls) {
                grouped.extend(group.renderers());
            }
        }
        Self { grouped }
    }

    pub(crate) fn holds(&self, id: u32, culls_small: bool) -> bool {
        culls_small || self.grouped.contains(&id)
    }
}

fn mix(key: u64, word: u64) -> u64 {
    key.wrapping_mul(0x100_0000_01b3).wrapping_add(word + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Word {
    /// A slot whose Render Face is not `Front`.
    Face {
        slot: u64,
        face: u64,
    },
    /// A Cutout slot, followed by its test's raw words.
    Cutout {
        slot: u64,
    },
    Raw(u32),
}

impl Word {
    fn of(self, id: u32) -> u64 {
        let id = u64::from(id);
        match self {
            Word::Face { slot, face } => (id << 18) | (slot << 2) | face,
            Word::Cutout { slot } => (id << 16) | slot,
            Word::Raw(w) => u64::from(w),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ObjectCasting {
    mode: Option<u64>,
    culls_small: bool,
    slots: Vec<Word>,
}

impl ObjectCasting {
    pub(crate) fn of(scene: &Scene, id: u32, mesh: &MeshComponent) -> Self {
        let mode = mesh.cast_shadows;
        let changes = !mode.casts() || mode == ShadowCasting::TwoSided;
        let mut slots = Vec::new();
        let count = scene.world.material(id).map_or(0, |m| m.materials.len());
        for slot in 0..count {
            let Some((_, asset)) = scene.material_entry_at(id, slot) else {
                continue;
            };
            let slot = slot as u64;
            if let Some(face) = face_word(asset.render_face) {
                slots.push(Word::Face { slot, face });
            }
            if let Some(cut) = Cutout::of(asset) {
                slots.push(Word::Cutout { slot });
                let map = asset.base_color_map.as_ref().map(MapRef::as_str);
                slots.extend(cutout_words(&cut, map).map(Word::Raw));
            }
        }
        Self {
            mode: changes.then_some(mode as u64),
            culls_small: mesh.culls_when_small(),
            slots,
        }
    }

    pub(crate) fn culls_small(&self) -> bool {
        self.culls_small
    }

    fn fold(&self, key: u64, id: u32, unbaked: &UnbakedCasters) -> u64 {
        let mut key = key;
        if let Some(mode) = self.mode {
            key = mix(key, (u64::from(id) << 2) | mode);
        }
        if unbaked.holds(id, self.culls_small) {
            key = mix(key, (u64::from(id) << 3) | 0b111);
        }
        self.slots.iter().fold(key, |k, w| mix(k, w.of(id)))
    }
}

pub(crate) fn casting_key<'a>(
    statics: impl Iterator<Item = (u32, &'a ObjectCasting)>,
    unbaked: &UnbakedCasters,
) -> u64 {
    statics.fold(0, |key, (id, casting)| casting.fold(key, id, unbaked))
}

fn face_word(face: RenderFace) -> Option<u64> {
    match face {
        RenderFace::Front => None,
        RenderFace::Back => Some(1),
        RenderFace::Both => Some(2),
    }
}

fn cutout_words<'a>(cut: &Cutout, map: Option<&'a str>) -> impl Iterator<Item = u32> + 'a {
    let words: Vec<u32> = bytemuck::bytes_of(&cut.mapping)
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .chain([cut.cutoff.to_bits()])
        .collect();
    words
        .into_iter()
        .chain(map.unwrap_or_default().bytes().map(u32::from))
}
