//! The lightmap report (#1980): written by the bake, a line per lightmapped mesh
//! with its texels and worker time, and its page's occupancy.

use super::tests::{quick, scene, scene_path};
use super::*;
use crate::dev::bake::report::{read, Kind};

#[test]
fn a_lightmap_bake_reports_each_mesh_and_page() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("report");
    let baked = bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let report = read(&path, Kind::Lightmaps).expect("the bake wrote its report");
    assert_eq!(report["maps"].as_u64(), Some(baked as u64));
    let meshes = report["meshes"].as_array().unwrap();
    assert_eq!(meshes.len(), baked);
    let mesh = &meshes[0];
    assert_eq!(mesh["entity"], floor);
    assert!(
        mesh["texels"].as_u64() > Some(0) && mesh["seconds"].as_f64().is_some(),
        "{mesh}"
    );
    assert!(mesh["charts"].as_u64() >= Some(1) && mesh["actualDensity"].as_f64().is_some());
    let occupancy = report["pages"][0]["occupancy"].as_f64().unwrap();
    assert!(occupancy > 0.0 && occupancy <= 1.0, "{}", report["pages"]);
    let names: Vec<_> = report["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].clone())
        .collect();
    assert_eq!(names, ["gather", "trace", "encode", "write"]);
    assert_eq!(
        report["outputBytes"].as_u64(),
        Some(crate::dev::bake::report::bytes_under(&lightmap_dir(&path)))
    );
}
