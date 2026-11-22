use super::*;
use crate::authoring::{create_entity, Primitive};
use rusty_components::{MaterialAsset, MaterialComponent};

fn fixture() -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, "Child", Some(Primitive::Sphere));
    let grandchild = create_entity(&mut scene, "Grandchild", Some(Primitive::Plane));
    scene.set_parent(child, Some(root)).unwrap();
    scene.set_parent(grandchild, Some(child)).unwrap();

    scene
        .materials
        .insert("Stone".into(), MaterialAsset::default());
    scene
        .world
        .set_material(root, Some(MaterialComponent::new("Stone")));
    (scene, root)
}

#[test]
fn extract_uses_local_zero_based_ids_and_detaches_root() {
    let (scene, root) = fixture();
    let prefab = extract_prefab(&scene, root).unwrap();

    assert_eq!(prefab.root, 0);
    assert_eq!(prefab.entities.len(), 3, "root + 2 descendants");
    assert_eq!(prefab.entities[0].id, 0);
    assert_eq!(prefab.entities[0].parent_id, None);
    assert_eq!(prefab.entities[0].children, vec![1]);
    assert_eq!(prefab.entities[1].id, 1);
    assert_eq!(prefab.entities[1].parent_id, Some(0));
    assert_eq!(prefab.entities[1].children, vec![2]);
    assert_eq!(prefab.entities[2].parent_id, Some(1));
}

#[test]
fn extract_slices_only_referenced_materials() {
    let (mut scene, root) = fixture();
    scene
        .materials
        .insert("Unused".into(), MaterialAsset::default());
    let prefab = extract_prefab(&scene, root).unwrap();
    assert!(prefab.materials.contains_key("Stone"));
    assert!(!prefab.materials.contains_key("Unused"));
}

#[test]
fn roundtrip_preserves_structure() {
    let (scene, root) = fixture();
    let prefab = extract_prefab(&scene, root).unwrap();
    let json = serde_json::to_string(&prefab).unwrap();
    let loaded: PrefabData = serde_json::from_str(&json).unwrap();

    let mut target = Scene::new();
    let new_root = instantiate_prefab(&mut target, &loaded, None);

    assert_eq!(target.entity_count(), 3);
    assert_eq!(*target.world.name(new_root).unwrap(), "Root");
    assert_eq!(target.world.parent_id(new_root), None);
    let children = target.world.children(new_root);
    assert_eq!(children.len(), 1);
    let child = children[0];
    assert_eq!(*target.world.name(child).unwrap(), "Child");
    assert_eq!(
        target.world.children(child).len(),
        1,
        "grandchild preserved"
    );
    assert!(!target.world.mesh(new_root).unwrap().vertices.is_empty());
}

#[test]
fn instantiate_assigns_fresh_sequential_deterministic_ids() {
    let (scene, root) = fixture();
    let prefab = extract_prefab(&scene, root).unwrap();

    // Two scenes seeded identically must yield identical ids.
    let make = || {
        let mut s = Scene::new();
        // Takes id 1, so the prefab's three entities get 2, 3 and 4.
        create_entity(&mut s, "Existing", None);
        let id = instantiate_prefab(&mut s, &prefab, None);
        (s.entity_ids(), id)
    };
    let (ids_a, root_a) = make();
    let (ids_b, root_b) = make();
    assert_eq!(ids_a, ids_b);
    assert_eq!(root_a, root_b);
    assert_eq!(ids_a, vec![1, 2, 3, 4]);
    assert_eq!(root_a, 2);
}

#[test]
fn instantiate_parents_new_root_under_requested_parent() {
    let (scene, root) = fixture();
    let prefab = extract_prefab(&scene, root).unwrap();
    let mut target = Scene::new();
    let host = create_entity(&mut target, "Host", None);
    let new_root = instantiate_prefab(&mut target, &prefab, Some(host));
    assert_eq!(target.world.parent_id(new_root), Some(host));
    assert!(target.world.children(host).contains(&new_root));
}

#[test]
fn identical_material_is_reused_conflict_is_uniquified() {
    let (scene, root) = fixture();
    let prefab = extract_prefab(&scene, root).unwrap();

    let mut a = Scene::new();
    a.materials.insert("Stone".into(), MaterialAsset::default());
    let ra = instantiate_prefab(&mut a, &prefab, None);
    assert_eq!(a.materials.len(), 1);
    assert_eq!(a.world.material(ra).unwrap().first(), "Stone");

    let mut b = Scene::new();
    let other = MaterialAsset {
        base_color: [0.1, 0.2, 0.3],
        ..Default::default()
    };
    b.materials.insert("Stone".into(), other);
    let rb = instantiate_prefab(&mut b, &prefab, None);
    assert_eq!(b.materials.len(), 2);
    let key = b.world.material(rb).unwrap().first().to_string();
    assert_ne!(key, "Stone");
    assert!(b.materials.contains_key(&key));
}

#[test]
fn is_prefab_path_recognises_extension() {
    assert!(is_prefab_path("assets/prefabs/Enemy.prefab"));
    assert!(is_prefab_path("X.PREFAB"));
    assert!(!is_prefab_path("foo.scene"));
}

#[test]
fn unpacked_instantiate_leaves_no_link_linked_stamps_one() {
    let (scene, root) = fixture();
    let prefab = extract_prefab(&scene, root).unwrap();

    let mut a = Scene::new();
    let ra = instantiate_prefab(&mut a, &prefab, None);
    assert!(!a.world.has_prefab_link(ra));

    let mut b = Scene::new();
    let rb = instantiate_prefab_linked(&mut b, &prefab, None, "Enemy.prefab");
    let link = b.world.prefab_link(rb).unwrap().clone();
    assert_eq!(link.source, "Enemy.prefab");
    assert_eq!(link.local_id, 0);
    assert!(
        link.overrides.is_empty(),
        "a fresh instance has no overrides"
    );
}
