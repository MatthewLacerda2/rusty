//! When the static casters' cached bake is stale: the key a cascade layer or an atlas
//! tile keeps beside what it baked, compared each frame (#355, #694).

use crate::asset::MapRef;
use crate::components::ShadowCasting;
use crate::render::passes::shadows::clips::Cutout;
use crate::scene::{Scene, SceneId};

/// Which scene's static casters a bake holds, at which of its
/// [`World::static_revision`](crate::ecs::World::static_revision)s, and with which
/// casting modes and cutout tests: a Static flag flipped, a static entity spawned or
/// despawned, a static mesh's `cast_shadows` changed (#1571), or a static Cutout
/// material's test or mapping edited (#1594) re-bakes them on the next frame whoever
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

/// One FNV-1a-style step of a digest.
fn mix(key: u64, word: u64) -> u64 {
    key.wrapping_mul(0x100_0000_01b3).wrapping_add(word + 1)
}

/// A digest, by entity in scene order, of what shapes each static mesh's shadow that
/// no revision tracks: a `cast_shadows` that changes it (`Off`, `TwoSided`, #1571;
/// `ShadowsOnly` casts exactly as `On`, so switching between them keeps the bake),
/// and each Cutout slot's test — cutoff, albedo map and mapping (#1594), the terms
/// its clip reads. It moves when one of them changes.
fn casting_key(scene: &Scene) -> u64 {
    let world = &scene.world;
    let mut key = 0u64;
    for id in world.ids_with_mesh() {
        if !world.is_static(id) {
            continue;
        }
        let mode = world.mesh(id).map_or(ShadowCasting::On, |m| m.cast_shadows);
        if !mode.casts() || mode == ShadowCasting::TwoSided {
            key = mix(key, (u64::from(id) << 2) | mode as u64);
        }
        for (slot, asset) in scene.material_assets_of(id).enumerate() {
            if let Some(cut) = Cutout::of(asset) {
                key = mix(key, (u64::from(id) << 16) | slot as u64);
                key = cutout_digest(key, &cut, asset.base_color_map.as_ref().map(MapRef::as_str));
            }
        }
    }
    key
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
