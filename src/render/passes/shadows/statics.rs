//! When the static casters' cached bake is stale: the key a cascade layer or an atlas
//! tile keeps beside what it baked, compared each frame (#355, #694).

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

/// `scene`'s [`StaticsKey`] now.
pub(crate) fn statics_key(scene: &Scene) -> StaticsKey {
    (
        scene.id(),
        scene.world.static_revision(),
        casting_key(scene),
    )
}

/// The meshes whose shadow never enters a cached static bake because a camera may
/// cull them as too far or too small (#1714): each that sets a small-object cull,
/// and **every** renderer of a `LODGroup` any of whose renderers sets one. A bake
/// draws a group at LOD0 (`LodSelection::finest`), so a group split between the
/// bake and the per-frame sweep would leave one level with no shadow, or two.
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

    /// Whether mesh entity `id` (`mesh`) casts from the per-frame sweep only.
    pub(crate) fn contains(&self, id: u32, mesh: &MeshComponent) -> bool {
        mesh.culls_when_small() || self.grouped.contains(&id)
    }
}

/// One FNV-1a-style step of a digest.
fn mix(key: u64, word: u64) -> u64 {
    key.wrapping_mul(0x100_0000_01b3).wrapping_add(word + 1)
}

/// A digest, by entity in scene order, of what shapes each static mesh's shadow that
/// no revision tracks: a `cast_shadows` that changes it (`Off`, `TwoSided`, #1571;
/// `ShadowsOnly` casts exactly as `On`, so switching between them keeps the bake),
/// whether it leaves the bake for a small-object cull ([`UnbakedCasters`], #1714),
/// each slot's Render Face other than `Front` (the faces it casts from, #1615),
/// and each Cutout slot's test — cutoff, albedo map and mapping (#1594), the terms
/// its clip reads. It moves when one of them changes.
fn casting_key(scene: &Scene) -> u64 {
    let world = &scene.world;
    let mut key = 0u64;
    let unbaked = UnbakedCasters::of(scene);
    for id in world.ids_with_mesh() {
        if !world.is_static(id) {
            continue;
        }
        let mesh = world.mesh(id);
        let mode = mesh.as_ref().map_or(ShadowCasting::On, |m| m.cast_shadows);
        if !mode.casts() || mode == ShadowCasting::TwoSided {
            key = mix(key, (u64::from(id) << 2) | mode as u64);
        }
        // A mesh a camera may cull as small casts from the per-frame sweep (#1714).
        if mesh.is_some_and(|m| unbaked.contains(id, &m)) {
            key = mix(key, (u64::from(id) << 3) | 0b111);
        }
        // By slot index, unresolved keys included, so the digest names the slot
        // `slot_clip` clips (`material_assets_of` would skip them and shift it).
        let slots = world.material(id).map_or(0, |m| m.materials.len());
        for (slot, (_, asset)) in
            (0..slots).filter_map(|s| Some((s, scene.material_entry_at(id, s)?)))
        {
            if let Some(face) = face_word(asset.render_face) {
                key = mix(key, (u64::from(id) << 18) | ((slot as u64) << 2) | face);
            }
            if let Some(cut) = Cutout::of(asset) {
                key = mix(key, (u64::from(id) << 16) | slot as u64);
                key = cutout_digest(key, &cut, asset.base_color_map.as_ref().map(MapRef::as_str));
            }
        }
    }
    key
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

/// `key` with `cut` and the albedo map it tests folded in.
fn cutout_digest(key: u64, cut: &Cutout, map: Option<&str>) -> u64 {
    let words = bytemuck::bytes_of(&cut.mapping)
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .chain([cut.cutoff.to_bits()]);
    let bytes = map.unwrap_or_default().bytes().map(u32::from);
    words.chain(bytes).fold(key, |k, w| mix(k, u64::from(w)))
}
