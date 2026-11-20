//! A kept occlusion bake (#2348) reports what its stamp sampled (#2328): a
//! navmesh that marked no cell is said so, with no floor share and no play
//! height, and one that did keeps both.

use crate::bake::occlusion::summary::MISSED;
use crate::bake::occlusion::BakedOcclusion;
use crate::bake::report::{occlusion, Stages};
use glam::Vec2;
use rusty_lighting::occlusion::test_level::corridors;
use rusty_lighting::occlusion::{gather_with, Fingerprints, OcclusionProgress, Reach};
use rusty_scene::lighting::floor::FloorGrid;

#[test]
fn a_kept_bake_reports_what_its_stamp_sampled() {
    let mut scene = corridors();
    let plan = FloorGrid::from_static(&scene, 0.5).expect("a floor");
    let mut far = plan.clone();
    far.origin += Vec2::splat(500.0);
    for (reach, missed) in [
        (Reach::over(&scene, &far), true),
        (Reach::over(&scene, &plan), false),
    ] {
        let input = gather_with(&scene, &Fingerprints::of_scene(&scene), reach.clone());
        let (data, r) = input
            .bake(2, &OcclusionProgress::default())
            .expect("not cancelled");
        assert_eq!(r.navmesh_missed, missed);
        scene.put_baked_occlusion(data);
        let Ok(kept) = BakedOcclusion::kept(&scene, &reach) else {
            panic!("current")
        };
        let k = &kept.report;
        assert_eq!(k.navmesh_missed, missed, "as its bake said");
        assert_eq!(
            k.floor_coverage.is_none(),
            missed,
            "a share only of a floor read"
        );
        let line = crate::bake::occlusion::summary::occlusion_summary(k, 0);
        assert_eq!(line.contains(MISSED), missed, "{line}");
        let report = occlusion::report(
            &scene,
            (k, scene.occlusion.as_deref(), (&[], None)),
            &Stages::start(),
            0,
        );
        let height = report.to_json()["settings"]["playHeight"].as_f64();
        assert_eq!(
            height,
            Some(if missed { 0.0 } else { 3.0 }),
            "the play height used"
        );
    }
}
