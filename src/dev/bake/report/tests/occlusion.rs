//! The occlusion report: on the corridor test level the PVS culls a share of the
//! objects, and Bake Occlusion writes the report beside the scene.

use super::{assert_common, scene_path};
use crate::dev::bake::generate_lighting::bake_occlusion_only;
use crate::dev::bake::progress::BakeLog;
use crate::dev::bake::report::{occlusion, read, Kind, Stages};
use crate::scene::lighting::occlusion::bake_occlusion;
use crate::scene::lighting::occlusion::test_level::corridors;

#[test]
fn the_corridors_cull_a_share_of_their_objects() {
    let scene = corridors();
    let (data, r) = bake_occlusion(&scene, 2);
    let data = data.expect("the corridors occlude");
    let report = occlusion::report(&scene, (&r, Some(&data)), &Stages::start(), 0).to_json();
    let v = &report["visibility"];
    let culled = v["culledFraction"].as_f64().unwrap();
    assert!(
        (0.2..1.0).contains(&culled),
        "the rooms hide each other: {v}"
    );
    let (mean, max) = (
        v["meanVisible"].as_f64().unwrap(),
        v["maxVisible"].as_f64().unwrap(),
    );
    assert!(mean <= max && max <= r.objects as f64, "{v}");
    let biggest = report["biggestOccluders"].as_array().unwrap();
    assert!(!biggest.is_empty() && report["occluderTriangles"].as_u64() > Some(0));
    assert!(
        report["slowestCells"].is_null(),
        "needs hooks inside the bake"
    );
}

#[test]
fn bake_occlusion_writes_its_report() {
    let path = scene_path("occlusion");
    let mut scene = corridors();
    let baked = bake_occlusion_only(&mut scene, Some(&path), &mut BakeLog::off()).unwrap();
    let report = read(&path, Kind::Occlusion).expect("the bake wrote its report");
    assert_common(&report, Kind::Occlusion);
    assert_eq!(report["cells"].as_u64(), Some(baked.occlusion.cells as u64));
    let size = std::fs::metadata(format!("{path}.occlusion"))
        .unwrap()
        .len();
    assert_eq!(report["outputBytes"].as_u64(), Some(size));
    assert_eq!(report["stages"][0]["name"], "bake");
}
