//! src/ecs/bundle.rs — document -> single-placement hecs bundle (#345).
//!
//! `world.rs`'s `place`/`overwrite` route every whole-entity write through
//! [`build_bundle`] so a document's mandatory + optional columns land in ONE
//! archetype placement, instead of N sequential `insert_one` calls that would
//! each re-migrate the entity through its own archetype (O(k) placement, not
//! O(k²) migration).

use crate::components::Entity;

/// The slim mandatory identity/hierarchy core every GameObject has (#343's
/// "core component"). Everything else — name, transform, scripts, and every
/// optional first-class component — is its own separately-attached hecs
/// column so narrow queries (#346) can skip entities that lack it.
pub(in crate::ecs) struct Core {
    pub id: u32,
    /// Monotonic insertion sequence, assigned by `World::place` and preserved
    /// across `overwrite`. Narrow queries (#346) sort their matches by it to
    /// recover insertion order — hecs iterates by archetype, and the
    /// determinism contract (physics pair ordering, replay byte-identity)
    /// requires insertion order.
    pub seq: u64,
    pub active: bool,
    pub is_static: bool,
    pub layer: u8,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

/// Attach each `Some` value in `$field` to the builder; a `None` simply
/// leaves that column unattached. Keeps `build_bundle` under the function
/// line cap despite the component list being long.
macro_rules! add_present {
    ($b:expr, $($field:expr),* $(,)?) => {
        $( if let Some(v) = $field { $b.add(v); } )*
    };
}

/// Build the full component bundle for one `Entity` document — the mandatory
/// core/name/transform/scripts plus every present optional component. Moves
/// fields directly off `entity` (a series of partial moves) rather than one
/// big destructure pattern, to stay well under the function line cap.
pub(in crate::ecs) fn build_bundle(entity: Entity, seq: u64) -> hecs::EntityBuilder {
    let mut b = hecs::EntityBuilder::new();
    b.add(Core {
        id: entity.id,
        seq,
        active: entity.active,
        is_static: entity.is_static,
        layer: entity.layer,
        parent_id: entity.parent_id,
        children: entity.children,
    });
    b.add(entity.name);
    b.add(entity.transform);
    b.add(entity.scripts);
    add_present!(
        b,
        entity.mesh,
        entity.material,
        entity.pending_material,
        entity.animator,
        entity.light,
        entity.collider,
        entity.rigidbody,
        entity.nav_agent,
        entity.camera,
        entity.visual_correction,
        entity.particles,
        entity.audio,
        entity.prefab_link,
    );
    b
}
