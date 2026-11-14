//! What a load's warm-up lists (#2123): each geometry slot once, from its newest revision,
//! each material map once as its slot reads it, the sky, and the lightmaps only
//! when the scene has pages.

use super::{Item, Warming};
use crate::gpu::textures::TextureKind;
use rusty_asset::MapRef;
use rusty_components::{MaterialComponent, MeshRevision};
use rusty_scene::authoring::{primitive_mesh_component, Primitive};
use rusty_scene::serialize::{to_scene_data, SceneData};
use rusty_scene::{MaterialAsset, Scene};

/// Two boxes and a sphere wearing `mapped`, whose albedo and normal maps are files,
/// and an entity with no mesh.
fn scene() -> Scene {
    let mut scene = Scene::new();
    for primitive in [Primitive::Box, Primitive::Box, Primitive::Sphere] {
        let id = scene.add_entity("Prop".into());
        scene
            .world
            .set_mesh(id, primitive_mesh_component(primitive));
        scene
            .world
            .set_material(id, Some(MaterialComponent::new("mapped")));
    }
    scene.add_entity("Empty".into());
    let mapped = MaterialAsset {
        base_color_map: Some(MapRef::from("albedo.png")),
        normal_map: Some(MapRef::from("normal.png")),
        ..MaterialAsset::default()
    };
    scene.materials.insert("mapped".into(), mapped);
    scene.skybox_path = String::new();
    scene
}

fn data() -> SceneData {
    to_scene_data(&scene())
}

#[test]
fn each_geometry_is_listed_once_by_its_newest_revision() {
    let mut data = data();
    // The second box edited since: its slot is uploaded from it.
    data.entities[1].mesh.as_mut().unwrap().revision = MeshRevision::next();
    let warming = Warming::plan(&data);
    let meshes: Vec<usize> = (warming.items.iter())
        .filter_map(|i| match i {
            Item::Mesh(at) => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(meshes.len(), 2, "one box, one sphere: {:?}", warming.items);
    assert_eq!(meshes[0], 1, "the box at its newest revision");
}

#[test]
fn each_map_is_listed_once_as_its_slot_reads_it_and_no_lightmaps_without_pages() {
    let warming = Warming::plan(&data());
    let maps: Vec<&Item> = (warming.items.iter())
        .filter(|i| matches!(i, Item::Texture(..)))
        .collect();
    let albedo = Item::Texture(MapRef::from("albedo.png"), TextureKind::Colour);
    let normal = Item::Texture(MapRef::from("normal.png"), TextureKind::Normal);
    assert_eq!(maps, [&albedo, &normal], "three wearers, two maps, no sky");
    assert!(!warming.items.contains(&Item::Lightmaps));
}

#[test]
fn a_sky_and_lightmap_pages_are_listed_and_progress_counts_what_is_made() {
    let mut data = data();
    data.skybox_path = "sky.hdr".into();
    data.lightmaps.pages = vec!["page_0.ktx2".into()];
    let mut warming = Warming::plan(&data);
    let sky = Item::Texture(MapRef::from("sky.hdr"), TextureKind::Colour);
    assert!(warming.items.contains(&sky), "as the panorama loads it");
    assert_eq!(
        warming.items.last(),
        Some(&Item::Lightmaps),
        "last: a worker"
    );
    assert_eq!((warming.progress(), warming.is_done()), (0.0, false));
    warming.next = warming.len() / 2;
    let half = warming.next as f32 / warming.len() as f32;
    assert_eq!(warming.progress(), half);
    warming.next = warming.len();
    assert_eq!((warming.progress(), warming.is_done()), (1.0, true));
}

#[test]
fn a_scene_with_nothing_to_draw_is_warm_at_once() {
    let mut empty = Scene::new();
    empty.skybox_path = String::new();
    let warming = Warming::plan(&to_scene_data(&empty));
    assert!(warming.is_empty() && warming.is_done());
    assert_eq!(warming.progress(), 1.0);
}
