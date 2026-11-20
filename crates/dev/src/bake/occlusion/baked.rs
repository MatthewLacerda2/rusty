//! A finished occlusion bake, or Generate Lighting's PVS kept unchanged (#2348):
//! what both runs hand back to be put on the scene and reported.

use std::time::{Duration, Instant};

use rusty_lighting::occlusion::{current_pvs, Fingerprints, OcclusionBakeReport, Reach};
use rusty_scene::lighting::occlusion::{FloorSource, OcclusionData};
use rusty_scene::Scene;

/// A finished occlusion bake: its PVS (`None` when nothing occludes or the level is too large to cull), its report,
/// the wall-clock milliseconds it took, and each cell's sampling time by cell
/// index (#1987; empty when no grid was laid). Or, when `kept`, the PVS the scene
/// already holds, unchanged (#2348): no `data` to apply, the held PVS's report,
/// the milliseconds the comparison took and no cell times. `standable` marks the
/// cells a camera can be in, whose rows are exact (`None`: every cell's is), which
/// the report's visibility figures are taken over.
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
    /// The PVS `scene` holds, kept, when it is what a bake sampling `reach`
    /// would write ([`current_pvs`]): Generate Lighting's one keep-or-bake
    /// decision, the editor's and the script's. Otherwise the fingerprints it
    /// compared with, for the bake to gather through ([`gather_with`]): no mesh is
    /// hashed twice.
    pub fn kept(scene: &Scene, reach: &Reach) -> Result<Self, Fingerprints> {
        let started = Instant::now();
        let held = current_pvs(scene, reach)?;
        let navmesh = held.stamp.floor == FloorSource::Navmesh;
        let report = OcclusionBakeReport {
            // What the held stamp's floor was: a kept bake that sampled every cell
            // over a navmesh that marked none says so, as its bake did.
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

    /// Put this bake on `scene`: its PVS, or, when kept, the file's ownership
    /// alone ([`Scene::keep_baked_occlusion`]), so the next save writes the kept
    /// PVS as a bake's would.
    pub fn put_on(self, scene: &mut Scene) {
        match self.kept {
            true => scene.keep_baked_occlusion(),
            false => scene.put_baked_occlusion(self.data),
        }
    }
}
