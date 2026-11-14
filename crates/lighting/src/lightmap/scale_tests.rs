//! Max Lightmap Size and Scale In Lightmap in the bake (#1704), Unity's rules: a
//! mesh's map follows its area × density up to the largest map a page holds; Scale
//! In Lightmap multiplies that density; at 0 the mesh gets no lightmap but still
//! blocks and bounces light.

use glam::Vec3;

use super::tests::{floor, settings};
use super::*;
use rusty_components::MeshComponent;

/// A `Baked` sun shining straight down.
fn noon() -> BakeLight {
    BakeLight {
        shape: LightShape::Directional {
            direction: Vec3::NEG_Y,
        },
        radiance: Vec3::splat(3.0),
        bakes_direct: true,
        mask_channel: None,
    }
}

/// The 4×4 floor and a copy of it 1 unit above, facing down, at `scale`.
fn floor_under_panel(scale: f32) -> BakeScene {
    let mut panel = floor();
    panel.entity = 3;
    panel.positions = panel.positions.iter().map(|p| *p + Vec3::Y).rev().collect();
    panel.normals = vec![Vec3::NEG_Y; 4];
    panel.scale_in_lightmap = scale;
    BakeScene {
        shadowmask: Default::default(),
        meshes: vec![floor(), panel],
        lights: vec![noon()],
        sky: Vec3::ZERO.into(),
        diffuse: Default::default(),
    }
}

fn size(maps: &[Lightmap], entity: u32) -> Option<u32> {
    maps.iter().find(|m| m.entity == entity).map(|m| m.size)
}

#[test]
fn a_map_follows_its_area_up_to_the_page_not_half_of_it() {
    // 4 m at 32 texels a unit asks for 128 texels; a 64 page holds 56.
    let dense = BakeSettings {
        texels_per_unit: 32.0,
        max_lightmap_size: 64,
        samples: 1,
        ..settings()
    };
    let scene = BakeScene {
        meshes: vec![floor()],
        ..Default::default()
    };
    let maps = bake(&scene, &dense);
    assert_eq!(size(&maps, 1), Some(max_map_edge(64)));
    assert_eq!(max_map_edge(64), 56, "the page less its ring");
    let atlas = pack(&maps, dense.page());
    assert_eq!((atlas.page_size, atlas.pages.len()), (64, 1), "it fits");
}

#[test]
fn scale_in_lightmap_multiplies_the_density() {
    let maps = bake(&floor_under_panel(2.0), &settings());
    // 4 m at 2 texels a unit: 8 for the floor, 16 for the panel at scale 2.
    assert_eq!((size(&maps, 1), size(&maps, 3)), (Some(8), Some(16)));
    let half = bake(&floor_under_panel(0.5), &settings());
    assert_eq!(
        size(&half, 3),
        Some(MIN_RESOLUTION),
        "never under the minimum"
    );
}

#[test]
fn at_zero_a_mesh_gets_no_lightmap_but_still_blocks_the_sun() {
    let maps = bake(&floor_under_panel(0.0), &settings());
    assert_eq!(size(&maps, 3), None, "no lightmap at scale 0");
    let floor = super::tests::mean(&maps, 1);
    assert!(floor < 0.1, "the panel still shades the floor: {floor}");
    assert!(!floor_under_panel(0.0).meshes[1].receives_lightmap());
}

#[test]
fn a_mesh_saved_without_the_field_reads_scale_one() {
    let read: MeshComponent = serde_json::from_str(r#"{ "primitive_type": "Box" }"#).unwrap();
    assert_eq!(read.scale_in_lightmap, 1.0);
    assert!(!read.reads_probes());
    let written = serde_json::to_value(&read).unwrap();
    assert_eq!(written["scale_in_lightmap"], 1.0, "always written");
}
