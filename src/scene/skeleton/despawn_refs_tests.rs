//! No reference outlives its entity (#1264): a scene holding every kind of entity
//! reference has each entity destroyed in turn, and afterwards no live reference
//! and no field of the saved document names a dead id. A new id-bearing field
//! joins its component's `remap_refs`, which is how this test finds it.

use std::cell::RefCell;
use std::collections::BTreeSet;

use super::fixture::hero;
use crate::components::lod_group::LodLevel;
use crate::components::ui::rect_transform::WorldAnchor;
use crate::components::{
    Entity, JointComponent, LodGroupComponent, ParticleEmitterComponent, RectTransformComponent,
    SelectableComponent,
};
use crate::ecs::World;
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::serialize::to_scene_data;
use crate::scene::Scene;

/// Every entity a document names: hierarchy links and component references.
fn named(doc: &Entity) -> Vec<u32> {
    let seen = RefCell::new(doc.children.clone());
    seen.borrow_mut().extend(doc.parent_id);
    let mut doc = doc.clone();
    doc.remap_refs(&|r| {
        seen.borrow_mut().push(r);
        Some(r)
    });
    seen.into_inner()
}

/// A hero with a gun on a bone and a rope to a bone; a parent → child → grandchild
/// chain each kind of reference points into; selection, `DontDestroyOnLoad` and a
/// lightmap entry on it too.
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
    referrers(&mut scene.world, [p, c, g], bone, gun);
    scene.selected_entity_id = Some(c);
    scene.persistent.insert(g);
    scene.lightmaps.entries.push(LightmapEntry {
        entity: p,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    scene
}

/// One entity per component reference kind, each naming one of `p, c, g`, `bone`
/// or `gun`.
fn referrers(w: &mut World, [p, c, g]: [u32; 3], bone: u32, gun: u32) {
    for (name, body) in [("Rope", p), ("BoneRope", bone)] {
        let joint = JointComponent {
            connected_body: Some(body),
            ..JointComponent::default()
        };
        let id = w.spawn(name.into());
        w.set_joint(id, Some(joint));
    }
    let mut sel = SelectableComponent {
        target_graphic: Some(c),
        ..SelectableComponent::default()
    };
    sel.select_on[0] = Some(g);
    let button = w.spawn("Button".into());
    w.set_selectable(button, Some(sel));
    let mut emitter = ParticleEmitterComponent::default();
    emitter.sub_emitters.birth = Some(p);
    emitter.sub_emitters.death = Some(gun);
    let fx = w.spawn("Fx".into());
    w.set_particles(fx, Some(emitter));
    let lod = LodGroupComponent {
        levels: vec![LodLevel {
            renderers: vec![c, g],
            ..LodLevel::default()
        }],
        ..LodGroupComponent::default()
    };
    let lod_id = w.spawn("Lod".into());
    w.set_lod_group(lod_id, Some(lod));
    let marker = RectTransformComponent {
        world_anchor: Some(WorldAnchor {
            target: Some(p),
            ..WorldAnchor::default()
        }),
        ..RectTransformComponent::default()
    };
    let marker_id = w.spawn("Marker".into());
    w.set_rect_transform(marker_id, Some(marker));
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
    assert!(ids.len() > 12, "the scene holds every kind: {ids:?}");
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
