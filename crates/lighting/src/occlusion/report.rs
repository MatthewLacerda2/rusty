//! What one occlusion bake found (#1980), the console summary's and the bake
//! report's source: its objects and cells, the cells it sampled and where it
//! found the floor (#2328), its rows and bytes. Read back from a PVS alone for a
//! bake kept unchanged (#2348).

use rusty_scene::lighting::occlusion::{FloorSource, OcclusionData};

/// What one occlusion bake found. All zero when it baked nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OcclusionBakeReport {
    /// Static meshes baked as occludees.
    pub objects: usize,
    /// Of them, the ones that occlude.
    pub occluders: usize,
    /// Of them, the ones too large to sample (`targets::MAX_TARGETS`) or with a
    /// corner that is not finite, kept in sight of every cell.
    pub never_culled: usize,
    pub cells: usize,
    /// Of them, the cells sampled (#2328); every other one sees every object.
    pub baked_cells: usize,
    /// Where the bake found the floor that decided which cells it sampled.
    pub floor: FloorSource,
    /// Whether a current navmesh was read but marked no cell of the grid, so
    /// every cell was sampled ([`FloorSource::Everywhere`]).
    pub navmesh_missed: bool,
    /// How far over the floor a camera can be, metres (`reach::play_height`); 0
    /// with no floor.
    pub play_height: f32,
    /// The share of the static bounds' footprint that floor covers (0 to 1),
    /// `None` with no floor: a small one reads as a stale or partial navmesh.
    pub floor_coverage: Option<f32>,
    /// The cells' edge, metres (0 when no grid was laid).
    pub cell_size: f32,
    /// Whether the level needs cells over `grid::MAX_CELL`, so nothing was baked
    /// and nothing is occlusion-culled.
    pub too_large: bool,
    /// Distinct visible sets the cells share.
    pub rows: usize,
    /// Bytes of the stored data.
    pub bytes: usize,
}

impl OcclusionBakeReport {
    /// The report of the bake that made `data`, read back from the PVS alone: what
    /// Generate Lighting says of a PVS it kept unchanged (#2348).
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
