//! What a bake refuses and what it does not blame (#1704): a bake past the page
//! limit fails before a page is drawn and leaves the scene's lightmaps as they
//! were; a mesh at Scale In Lightmap 0 is not counted as missing a lightmap UV.

use glam::Vec3;

use super::tests::{quick, scene};
use super::*;
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::lighting::lightmap::{
    bake, check_page_count, max_map_edge, Lightmap, LightmapEntry, MAX_PAGES,
};

/// `n` lightmaps each as large as a 32-texel page holds: one page apiece.
fn page_fillers(n: usize) -> Vec<Lightmap> {
    let size = max_map_edge(32);
    (0..n as u32)
        .map(|entity| Lightmap {
            entity,
            size,
            texels: vec![Vec3::ONE; (size * size) as usize],
            directions: Vec::new(),
        })
        .collect()
}

#[test]
fn a_bake_past_the_page_limit_names_the_count_the_limit_and_the_fix() {
    let maps = page_fillers(3);
    let refused = check_page_count(&maps, 32, 2).unwrap_err();
    assert!(refused.contains("3 lightmap pages of 32²"), "{refused}");
    assert!(refused.contains("the 2 a lightmap"), "{refused}");
    assert!(refused.contains("raise Max Lightmap Size"), "{refused}");
    assert_eq!(check_page_count(&maps, 32, 3), Ok(()));
    assert_eq!(
        check_page_count(&maps, 64, 2),
        Ok(()),
        "a larger page holds them"
    );
}

#[test]
fn a_refused_bake_keeps_the_scene_s_previous_lightmaps() {
    let (mut scene, floor, _) = scene();
    scene.lightmaps.insert(LightmapEntry {
        entity: floor,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    scene.lighting_settings.lightmaps.max_lightmap_size = 32;
    let before = scene.lightmaps.clone();
    let path = crate::test_temp::dir().join("refused_1704.scene");
    let path = path.to_string_lossy();
    let refused = apply_lightmaps(&mut scene, &path, &page_fillers(MAX_PAGES + 1));
    assert!(refused.unwrap_err().contains("257 lightmap pages"));
    assert_eq!(scene.lightmaps, before, "nothing applied");
    assert!(!lightmap_dir(&path).exists(), "nothing written");
}

#[test]
fn a_scale_zero_mesh_is_not_blamed_for_a_missing_uv() {
    let (mut scene, floor, _) = scene();
    let mut mesh = primitive_mesh_component(Primitive::Plane).unwrap();
    mesh.scale_in_lightmap = 0.0;
    scene.world.set_mesh(floor, Some(mesh));
    let input = gather_bake_input(&scene);
    let maps = bake(&input, &quick());
    assert!(
        maps.is_empty(),
        "the floor is at scale 0, the ball has no UV"
    );
    assert_eq!(lightmap_receivers(&input), 1, "only the ball asks for one");
    let report = missing_uv_report(lightmap_receivers(&input), maps.len());
    assert!(report.unwrap().starts_with("1 static mesh has"));
}
