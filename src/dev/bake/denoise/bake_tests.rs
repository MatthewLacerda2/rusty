//! A whole bake through real OIDN (#1884), skipped where OIDN is missing: the same
//! bytes whatever order the workers finish their charts in, and on whichever of
//! the bake's devices each chart lands, and each chart counted once.

use glam::Vec3;

use super::tests::oidn_or_skip;
use super::Oidn;
use crate::scene::lighting::lightmap::{
    bake_with_progress, BakeLight, BakeProgress, BakeScene, BakeSettings, LightShape,
};
use crate::scene::lighting::occlusion::bench::level::rooms;

#[test]
fn a_whole_oidn_bake_repeats_its_bytes_whichever_worker_finishes_first() {
    let Some(_) = oidn_or_skip() else { return };
    let mut scene = BakeScene::gather(&rooms(1), &|_| None);
    let position = Vec3::new(3.0, 3.0, 3.0);
    let shape = LightShape::Point {
        position,
        range: 10.0,
    };
    let (radiance, bakes_direct, mask_channel) = (Vec3::splat(3.0), true, None);
    scene.lights.push(BakeLight {
        shape,
        radiance,
        bakes_direct,
        mask_channel,
    });
    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 4,
        ..Default::default()
    };
    let bake = || {
        let oidn = std::sync::Arc::new(Oidn::default());
        let progress = BakeProgress::default().with_denoiser(oidn.clone());
        let maps = bake_with_progress(&scene, &settings, &progress).expect("not cancelled");
        let charts = maps.len() as u64;
        assert_eq!(
            oidn.spent().0,
            charts,
            "each chart counted once, directions too"
        );
        maps
    };
    assert_eq!(bake(), bake(), "the same bytes, colour and directions");
}
