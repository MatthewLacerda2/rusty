//! When the static casters' cached bake is stale: the key a cascade layer or an atlas
//! tile keeps beside what it baked, compared each frame (#355, #694).
//!
//! The digest is folded from the retained scene's objects (#1706): each keeps its
//! [`ObjectCasting`], and the scene refolds the digest only on an extract that
//! re-derived a static object or changed which renderers a `LODGroup` keeps out of
//! the bake ([`UnbakedCasters`]). [`statics_key`] folds the same words from the ECS,
//! from scratch: what the tests compare the kept key against.

use std::collections::HashSet;

use crate::asset::MapRef;
use crate::components::{MeshComponent, RenderFace, ShadowCasting};
use crate::render::passes::shadows::clips::Cutout;
use crate::scene::{Scene, SceneId};

/// Which scene's static casters a bake holds, at which of its
/// [`World::static_revision`](crate::ecs::World::static_revision)s, and with which
/// casting modes, render faces and cutout tests: a Static flag flipped, a static
/// entity spawned or despawned, a static mesh's `cast_shadows` changed (#1571) or
/// its small-object cull set or cleared (#1714), a static material's Render Face switched (#1615), or a static Cutout material's
/// test or mapping edited (#1594) re-bakes them on the next frame whoever
/// made the change — the inspector, a script in Play, a load (#1496).
pub(crate) type StaticsKey = (SceneId, u64, u64);

/// `scene`'s [`StaticsKey`] now, read from the ECS from scratch. The renderer keys
/// its bakes with the retained scene's (`RenderScene::statics_key`), which equals it.
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

/// The meshes whose shadow never enters a cached static bake because a camera may
/// cull them as too far or too small (#1714): each that sets a small-object cull,
/// and **every** renderer of a `LODGroup` any of whose renderers sets one. A bake
/// draws a group at LOD0 (`LodSelection::finest`), so a group split between the
/// bake and the per-frame sweep would leave one level with no shadow, or two.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UnbakedCasters {
    /// The renderers of the groups that hold such a mesh.
    grouped: HashSet<u32>,
}

impl UnbakedCasters {
    /// `scene`'s groups that hold a mesh with a small-object cull, read now.
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

    /// Whether mesh entity `id`, which sets a small-object cull when `culls_small`
    /// ([`MeshComponent::culls_when_small`]), casts from the per-frame sweep only.
    pub(crate) fn holds(&self, id: u32, culls_small: bool) -> bool {
        culls_small || self.grouped.contains(&id)
    }
}

/// One FNV-1a-style step of a digest.
fn mix(key: u64, word: u64) -> u64 {
    key.wrapping_mul(0x100_0000_01b3).wrapping_add(word + 1)
}

/// One word a static mesh's slots add to the digest, its entity id not yet folded in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Word {
    /// A slot whose Render Face is not `Front` (#1615).
    Face {
        slot: u64,
        face: u64,
    },
    /// A Cutout slot (#1594), followed by its test's raw words.
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

/// What shapes one mesh's static shadow that no revision tracks, read once when its
/// retained object is derived (#1706): a `cast_shadows` that changes it (`Off`,
/// `TwoSided`, #1571; `ShadowsOnly` casts exactly as `On`, so switching between
/// them keeps the bake), whether it sets a small-object cull (#1714), each slot's
/// Render Face other than `Front` (the faces it casts from, #1615), and each Cutout
/// slot's test — cutoff, albedo map and mapping (#1594), the terms its clip reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ObjectCasting {
    mode: Option<u64>,
    culls_small: bool,
    slots: Vec<Word>,
}

impl ObjectCasting {
    /// Mesh entity `id`'s (`mesh`) terms in `scene` now.
    pub(crate) fn of(scene: &Scene, id: u32, mesh: &MeshComponent) -> Self {
        let mode = mesh.cast_shadows;
        let changes = !mode.casts() || mode == ShadowCasting::TwoSided;
        let mut slots = Vec::new();
        // By slot index, unresolved keys included, so the digest names the slot
        // the shadow clips (`material_assets_of` would skip them and shift it).
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

    /// Whether it sets a small-object cull of its own.
    pub(crate) fn culls_small(&self) -> bool {
        self.culls_small
    }

    /// `key` with entity `id`'s terms folded in, `unbaked` its verdict.
    fn fold(&self, key: u64, id: u32, unbaked: &UnbakedCasters) -> u64 {
        let mut key = key;
        if let Some(mode) = self.mode {
            key = mix(key, (u64::from(id) << 2) | mode);
        }
        // A mesh a camera may cull as small casts from the per-frame sweep (#1714).
        if unbaked.holds(id, self.culls_small) {
            key = mix(key, (u64::from(id) << 3) | 0b111);
        }
        self.slots.iter().fold(key, |k, w| mix(k, w.of(id)))
    }
}

/// The digest, by entity in scene order, of the static meshes' [`ObjectCasting`]s:
/// it moves when one of their terms changes.
pub(crate) fn casting_key<'a>(
    statics: impl Iterator<Item = (u32, &'a ObjectCasting)>,
    unbaked: &UnbakedCasters,
) -> u64 {
    statics.fold(0, |key, (id, casting)| casting.fold(key, id, unbaked))
}

/// A non-`Front` face's digest word; `Front`, the default, adds none, so the key of
/// a scene with no two-sided material is what it was before faces existed.
fn face_word(face: RenderFace) -> Option<u64> {
    match face {
        RenderFace::Front => None,
        RenderFace::Back => Some(1),
        RenderFace::Both => Some(2),
    }
}

/// The words of `cut` and the albedo map it tests.
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
