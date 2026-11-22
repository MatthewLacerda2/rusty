use rusty_scene::lighting::occlusion::{FloorSource, OcclusionData};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OcclusionBakeReport {
    pub objects: usize,
    pub occluders: usize,
    pub never_culled: usize,
    pub cells: usize,
    pub baked_cells: usize,
    pub floor: FloorSource,
    pub navmesh_missed: bool,
    pub play_height: f32,
    pub floor_coverage: Option<f32>,
    pub cell_size: f32,
    pub too_large: bool,
    pub rows: usize,
    pub bytes: usize,
}

impl OcclusionBakeReport {
    pub fn of_kept(data: &OcclusionData) -> Self {
        Self {
            objects: data.objects.len(),
            occluders: data.objects.iter().filter(|o| o.occluder).count(),
            never_culled: data.stamp.never_culled as usize,
            cells: data.grid.cell_count(),
            baked_cells: data.stamp.baked as usize,
            floor: data.stamp.floor,
            navmesh_missed: false,
            play_height: 0.0,
            floor_coverage: None,
            cell_size: data.grid.cell,
            too_large: false,
            rows: data.row_count(),
            bytes: rusty_scene::lighting::occlusion::encoded_len(data),
        }
    }
}
