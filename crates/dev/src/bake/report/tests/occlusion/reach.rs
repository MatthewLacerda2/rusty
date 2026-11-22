use super::super::{assert_common, scene_path};
use crate::bake::generate_lighting::bake_occlusion_only;
use crate::bake::occlusion::reach;
use crate::bake::progress::BakeLog;
use crate::bake::report::{occlusion, read, Kind, Stages, TOP};
use rusty_lighting::occlusion::test_level::corridors;
use rusty_lighting::occlusion::{bake_occlusion, gather_with, Fingerprints, OcclusionProgress};
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::occlusion::FloorSource;

#[test]
fn the_visibility_over_the_standable_cells_is_a_full_bakes_there() {
    let scene = corridors();
    let mut nav = NavigationGraph::from_scene(&scene);
    nav.bake(&scene);
    let reach = reach(&scene, Some(&nav));
    assert_eq!(reach.source, FloorSource::Navmesh, "a current navmesh");
    let input = gather_with(&scene, &Fingerprints::of_scene(&scene), reach.clone());
    let progress = OcclusionProgress::default();
    let (data, r) = input.bake(2, &progress).expect("not cancelled");
    let data = data.expect("occluders");
    let (full, f) = bake_occlusion(&scene, 2);
    let full = full.expect("occluders");
    assert_eq!(data.grid, full.grid);
    assert!(r.baked_cells < r.cells, "{} of {}", r.baked_cells, r.cells);
    let standable = reach.standable(&data.grid).expect("a floor");
    let at = |d, r| {
        let measured = (&[][..], Some(&standable[..]));
        occlusion::report(&scene, (r, Some(d), measured), &Stages::start(), 0).to_json()
    };
    let (ours, theirs) = (at(&data, &r), at(&full, &f));
    assert_eq!(ours["visibility"], theirs["visibility"]);
    let culled = ours["visibility"]["culledFraction"].as_f64().unwrap();
    assert!(culled >= 0.2, "{}", ours["visibility"]);
    let coverage = ours["floorCoverage"].as_f64().expect("a floor's share");
    assert!(coverage > 0.0 && coverage <= 1.0, "{coverage}");
}

#[test]
fn a_stale_or_missing_navmesh_samples_every_cell_and_says_so() {
    let mut scene = corridors();
    let mut nav = NavigationGraph::from_scene(&scene);
    nav.bake(&scene);
    let crate_id = scene.world.find_by_name("Crate").expect("a crate");
    scene
        .world
        .transform_mut(crate_id)
        .expect("crate")
        .position
        .x += 3.0;
    assert_eq!(
        reach(&scene, Some(&nav)).source,
        FloorSource::Everywhere,
        "stale"
    );
    assert_eq!(reach(&scene, None).source, FloorSource::Everywhere);
    let path = scene_path("occlusion_stale_nav");
    let baked = bake_occlusion_only(&mut scene, Some(&path), Some(&nav), &mut BakeLog::off());
    let baked = baked.unwrap();
    assert_eq!(baked.occlusion.baked_cells, baked.occlusion.cells);
    let report = read(&path, Kind::Occlusion).expect("the bake wrote its report");
    assert_eq!(report["floorSource"], "everywhere");
    assert!(report["floorCoverage"].is_null());
    let line = crate::bake::occlusion::summary::occlusion_summary(&baked.occlusion, 0);
    assert!(
        line.contains("no fresh navmesh: every cell sampled; bake the navmesh"),
        "{line}"
    );
}

#[test]
fn bake_occlusion_writes_its_report() {
    let path = scene_path("occlusion");
    let mut scene = corridors();
    let baked = bake_occlusion_only(&mut scene, Some(&path), None, &mut BakeLog::off()).unwrap();
    let report = read(&path, Kind::Occlusion).expect("the bake wrote its report");
    assert_common(&report, Kind::Occlusion);
    assert_eq!(report["cells"].as_u64(), Some(baked.occlusion.cells as u64));
    assert_eq!(
        report["floorSource"], "everywhere",
        "no navmesh: every cell"
    );
    let (sampled, skipped) = (
        report["bakedCells"].as_u64(),
        report["unbakedCells"].as_u64(),
    );
    assert_eq!(
        sampled.unwrap() + skipped.unwrap(),
        baked.occlusion.cells as u64
    );
    let layers = report["layerSeconds"].as_array().expect("timed");
    let per_layer: u64 = layers.iter().map(|l| l["cells"].as_u64().unwrap()).sum();
    assert_eq!(
        Some(per_layer),
        sampled,
        "the layers count the sampled cells"
    );
    let size = std::fs::metadata(format!("{path}.occlusion"))
        .unwrap()
        .len();
    assert_eq!(report["outputBytes"].as_u64(), Some(size));
    assert_eq!(report["stages"][0]["name"], "bake");
    let slowest = report["slowestCells"]
        .as_array()
        .expect("the bake timed its cells");
    assert!(!slowest.is_empty() && slowest.len() <= TOP);
    let secs: Vec<f64> = slowest
        .iter()
        .map(|c| c["seconds"].as_f64().unwrap())
        .collect();
    assert!(
        secs.windows(2).all(|w| w[0] >= w[1]),
        "slowest first: {secs:?}"
    );
    assert_eq!(report["cellSeconds"]["max"].as_f64(), Some(secs[0]));
}
