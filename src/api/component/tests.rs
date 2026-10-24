//! The field table can't fall behind (#1369): every first-class component's
//! serialized fields are in its table, each mapped to an op or listed read-only.

use std::collections::BTreeSet;

use serde_json::Value;

use super::introspect::serde_fields;
use super::kinds::KINDS;
use super::table::Kind;
use super::write::Skip;
use crate::components::animator::Playback;
use crate::components::Entity;
use crate::scene::authoring::{add_component, ComponentKind};
use crate::scene::Scene;

/// `Entity`'s saved fields that are not a component, each with why.
const NOT_COMPONENTS: &[&str] = &[
    "id",
    "name",
    "active",
    "is_static",
    "layer",       // the GameObject's own header
    "scripts",     // many per entity, by path: Scene.AddScript / GetScript
    "prefab_link", // the instance's link to its .prefab, not a component
    "parent_id",
    "parent_bone",
    "bone_refs",
    "children", // the hierarchy
];

fn names(kind: &Kind) -> BTreeSet<&'static str> {
    (kind.fields)().into_iter().map(|(n, _)| n).collect()
}

#[test]
fn every_entity_component_has_a_kind() {
    let fields = serde_fields::<Entity>().expect("Entity derives Deserialize");
    let components: BTreeSet<&str> = fields
        .iter()
        .copied()
        .filter(|f| !NOT_COMPONENTS.contains(f))
        .collect();
    let keys: BTreeSet<&str> = KINDS.iter().map(|k| k.key).collect();
    assert_eq!(components, keys, "Entity's components vs Component.* kinds");
    for add in ComponentKind::ALL {
        let n = KINDS.iter().filter(|k| k.add == Some(*add)).count();
        assert_eq!(n, 1, "{add:?} has {n} kinds");
    }
}

#[test]
fn every_serialized_field_is_mapped_or_read_only() {
    for kind in KINDS {
        let table = names(kind);
        assert_eq!(
            table.len(),
            (kind.fields)().len(),
            "{}: a field listed twice",
            kind.name
        );
        match (kind.serde_fields)() {
            Some(saved) => {
                let saved: BTreeSet<&str> = saved.iter().copied().collect();
                assert_eq!(table, saved, "{}: table vs serialized fields", kind.name);
            }
            // Animator flattens its `Playback`, so serde reads it as a map: check
            // the flattened fields and everything a default one saves.
            None => {
                assert_eq!(kind.name, "Animator", "only Animator flattens");
                let playback = serde_fields::<Playback>().expect("Playback is a struct");
                let missing: Vec<_> = playback.iter().filter(|f| !table.contains(*f)).collect();
                assert!(missing.is_empty(), "Animator misses {missing:?}");
            }
        }
    }
}

/// An entity carrying every component the Add Component menu can attach.
fn dressed() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Dressed".to_string());
    for add in ComponentKind::ALL {
        add_component(&mut scene, id, *add);
    }
    let mesh =
        crate::scene::authoring::primitive_mesh_component(crate::scene::authoring::Primitive::Box);
    scene.world.set_mesh(id, mesh);
    (scene, id)
}

#[test]
fn each_kind_reads_its_entity_key() {
    let (scene, id) = dressed();
    let doc = serde_json::to_value(scene.world.entity_document(id).unwrap()).unwrap();
    for kind in KINDS {
        let read = (kind.read)(&scene.world, id).expect(kind.name).unwrap();
        assert_eq!(read, doc[kind.key], "{} reads `{}`", kind.name, kind.key);
        let saved: BTreeSet<&str> = read
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let table = names(kind);
        let extra: Vec<_> = saved.difference(&table).collect();
        assert!(
            extra.is_empty(),
            "{}: saved but not in the table: {extra:?}",
            kind.name
        );
    }
}

/// Each op, driven with the field's own serialized value (no unchanged-key skip),
/// leaves the component as it was: an op that can't decode what `Get` reads fails.
#[test]
fn every_writable_field_takes_back_what_get_reads() {
    let (mut scene, id) = dressed();
    for kind in KINDS {
        let before = (kind.read)(&scene.world, id).unwrap().unwrap();
        let Value::Object(fields) = &before else {
            panic!("{} is not a table", kind.name)
        };
        for (name, read_only) in (kind.fields)() {
            let Some(value) = fields.get(name).filter(|_| read_only.is_none()) else {
                continue;
            };
            let one = serde_json::Map::from_iter([(name.to_string(), value.clone())]);
            let written = (kind.write)(&mut scene.world, id, &one, Skip::Nothing);
            assert_eq!(written, Ok(true), "{}.{name} = {value}", kind.name);
        }
        let after = (kind.read)(&scene.world, id).unwrap().unwrap();
        assert_eq!(
            before, after,
            "{}: writing its own values changed it",
            kind.name
        );
    }
}
