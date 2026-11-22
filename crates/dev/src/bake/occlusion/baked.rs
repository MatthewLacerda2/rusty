use std::time::{Duration, Instant};

use rusty_lighting::occlusion::{current_pvs, Fingerprints, OcclusionBakeReport, Reach};
use rusty_scene::lighting::occlusion::{FloorSource, OcclusionData};
use rusty_scene::Scene;

#[derive(Debug)]
pub struct BakedOcclusion {
    pub data: Option<OcclusionData>,
    pub report: OcclusionBakeReport,
    pub ms: u64,
    pub cells: Vec<Option<Duration>>,
    pub standable: Option<Vec<bool>>,
    pub kept: bool,
}

impl BakedOcclusion {
    /// `Err` hands back the fingerprints compared, so the bake gathers through them: no mesh
    /// is hashed twice.
    pub fn kept(scene: &Scene, reach: &Reach) -> Result<Self, Fingerprints> {
        let started = Instant::now();
        let held = current_pvs(scene, reach)?;
        let navmesh = held.stamp.floor == FloorSource::Navmesh;
        let report = OcclusionBakeReport {
            floor_coverage: reach.coverage(held.stamp.bounds).filter(|_| navmesh),
            navmesh_missed: !navmesh && reach.source == FloorSource::Navmesh,
            play_height: if navmesh { reach.play_height } else { 0.0 },
            ..OcclusionBakeReport::of_kept(held)
        };
        Ok(Self {
            data: None,
            report,
            ms: started.elapsed().as_millis() as u64,
            cells: Vec::new(),
            standable: reach.standable(&held.grid),
            kept: true,
        })
    }

    pub fn put_on(self, scene: &mut Scene) {
        match self.kept {
            true => scene.keep_baked_occlusion(),
            false => scene.put_baked_occlusion(self.data),
        }
    }
}
