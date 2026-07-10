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
    pub active: bool,
    pub is_static: bool,
    pub layer: u8,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

/// Build the full component bundle for one `Entity` document — the mandatory
/// core/name/transform/scripts plus every present optional component.
pub(in crate::ecs) fn build_bundle(entity: Entity) -> hecs::EntityBuilder {
    let Entity {
        id,
        name,
        active,
        is_static,
        layer,
        transform,
        mesh,
        material,
        pending_material,
        scripts,
        animator,
        light,
        collider,
        rigidbody,
        nav_agent,
        camera,
        visual_correction,
        particles,
        audio,
        prefab_link,
        parent_id,
        children,
    } = entity;
    let mut b = hecs::EntityBuilder::new();
    b.add(Core {
        id,
        active,
        is_static,
        layer,
        parent_id,
        children,
    });
    b.add(name);
    b.add(transform);
    b.add(scripts);
    if let Some(v) = mesh {
        b.add(v);
    }
    if let Some(v) = material {
        b.add(v);
    }
    if let Some(v) = pending_material {
        b.add(v);
    }
    if let Some(v) = animator {
        b.add(v);
    }
    if let Some(v) = light {
        b.add(v);
    }
    if let Some(v) = collider {
        b.add(v);
    }
    if let Some(v) = rigidbody {
        b.add(v);
    }
    if let Some(v) = nav_agent {
        b.add(v);
    }
    if let Some(v) = camera {
        b.add(v);
    }
    if let Some(v) = visual_correction {
        b.add(v);
    }
    if let Some(v) = particles {
        b.add(v);
    }
    if let Some(v) = audio {
        b.add(v);
    }
    if let Some(v) = prefab_link {
        b.add(v);
    }
    b
}
