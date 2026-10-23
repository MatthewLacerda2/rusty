//! No reference outlives its entity (#1264): a scene holding every kind of entity
//! reference has each entity destroyed in turn, and afterwards no live reference
//! and no field of the saved document names a dead id. The references are built
//! and read through the one id-field walk (`components::entity::refs`), so a new
//! kind is covered the moment it joins the walk.

use std::collections::BTreeSet;

use super::fixture::hero;
use crate::components::{Entity, EntityRefs};
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::serialize::to_scene_data;
use crate::scene::Scene;

/// Every entity a document names: hierarchy links and component references.
fn named(doc: &Entity) -> Vec<u32> {
    let mut seen = doc.children.clone();
    seen.extend(doc.parent_id);
    doc.each_ref(&mut |_, r| seen.push(r));
    seen
}

/// An entity carrying every reference-holding component, every slot naming
/// `target`. Returns the components the walk visited (the pointers' first token).
fn referrer(scene: &mut Scene, target: u32) -> BTreeSet<String> {
    let mut doc = Entity::every_ref();
    doc.id = scene.world.next_id();
    let mut kinds = BTreeSet::new();
    doc.visit_refs(&mut |pointer, r| {
        kinds.insert(pointer.split('/').nth(1).unwrap_or_default().to_string());
        *r = Some(target);
    });
    scene.world.insert_entity(doc);
    kinds
}

/// A hero with a gun on a bone; a parent → child → grandchild chain; one referrer
/// naming each of those, the bone and the gun; selection, `DontDestroyOnLoad` and
/// a lightmap entry on the chain too.
fn every_kind(tag: &str) -> Scene {
    let mut scene = Scene::new();
    let hero = hero(&mut scene, tag);
    let bone = scene.find_bone(hero, "Joint1").unwrap();
    let gun = scene.add_entity("Gun".into());
    scene.set_parent(gun, Some(bone)).unwrap();
    let (p, c, g) = (
        scene.add_entity("P".into()),
        scene.add_entity("C".into()),
        scene.add_entity("G".into()),
    );
    scene.set_parent(c, Some(p)).unwrap();
    scene.set_parent(g, Some(c)).unwrap();
    for target in [p, c, g, bone, gun] {
        referrer(&mut scene, target);
    }
    scene.selected_entity_id = Some(c);
    scene.persistent.insert(g);
    scene.lightmaps.entries.push(LightmapEntry {
        entity: p,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    scene
}

#[test]
fn the_walk_reaches_every_reference_holding_component() {
    let mut scene = Scene::new();
    let kinds = referrer(&mut scene, 0);
    // Every component the fixture carries, read off its saved form.
    let doc = serde_json::to_value(Entity::every_ref()).unwrap();
    let carried: BTreeSet<String> = doc
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, v)| v.is_object() && *key != "transform")
        .map(|(key, _)| key.clone())
        .collect();
    assert_eq!(kinds, carried, "each listed component has a slot");
    let mut pointers = Vec::new();
    Entity::every_ref().visit_refs(&mut |p, _| pointers.push(p.to_string()));
    for p in pointers.iter().filter(|p| !p.starts_with("/prefab_link/")) {
        assert!(Entity::is_ref_pointer(p), "{p} is a reference pointer");
    }
}

/// Every id the scene names, live: hierarchy, components, and scene-level state.
fn live_refs(scene: &Scene) -> Vec<u32> {
    let mut out: Vec<u32> = scene
        .entity_ids()
        .into_iter()
        .filter_map(|id| scene.world.entity_document(id))
        .flat_map(|doc| named(&doc))
        .collect();
    out.extend(scene.selected_entity_id);
    out.extend(scene.persistent.iter().copied());
    out.extend(scene.pending_destroy.iter().copied());
    out.extend(scene.lightmaps.entries.iter().map(|e| e.entity));
    out
}

#[test]
fn destroying_any_entity_leaves_no_reference_to_it() {
    let ids = every_kind("refs_all").entity_ids();
    assert!(ids.len() >= 12, "the scene holds every kind: {ids:?}");
    for (n, &dead) in ids.iter().enumerate() {
        let mut scene = every_kind(&format!("refs_{n}"));
        scene.request_destroy(dead);
        scene.destroy_entity(dead);
        let dangling: Vec<u32> = live_refs(&scene)
            .into_iter()
            .filter(|&r| !scene.world.contains(r))
            .collect();
        assert!(
            dangling.is_empty(),
            "destroying {dead}: live refs {dangling:?}"
        );

        let data = to_scene_data(&scene);
        let saved: BTreeSet<u32> = data.entities.iter().map(|e| e.id).collect();
        let mut named_saved: Vec<u32> = data.entities.iter().flat_map(named).collect();
        named_saved.extend(data.selected_entity_id);
        named_saved.extend(data.lightmaps.entries.iter().map(|e| e.entity));
        named_saved.retain(|r| !saved.contains(r));
        assert!(
            named_saved.is_empty(),
            "destroying {dead}: saved refs {named_saved:?}"
        );
    }
}
