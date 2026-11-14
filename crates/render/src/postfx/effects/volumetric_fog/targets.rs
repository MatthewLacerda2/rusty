//! One view's volumetric fog state (#1703), held in `PostFx::targets` while its base
//! camera runs it: the two scatter grids the frames alternate between (this frame's
//! and its history), the uniform, and what the history was built from. Made for
//! one grid and rebuilt (history and all) when the grid changes; a resize drops it
//! with every slot.

use super::params::Built;
use super::volume::froxel_grid;
use crate::motion::HistoryKey;

/// What last frame's grid holds, and for which cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct History {
    pub key: HistoryKey,
    pub built: Built,
}

pub(crate) struct VolumetricTargets {
    grid: [u32; 3],
    pub(super) scatter: [wgpu::TextureView; 2],
    pub(super) uniform: wgpu::Buffer,
    /// Which of `scatter` this frame writes.
    pub(super) current: usize,
    /// Frames run, modulo the jitter's period.
    pub(super) index: u32,
    pub(super) history: Option<History>,
}

/// Frames the jitter runs before it repeats.
const PERIOD: u32 = 64;

impl VolumetricTargets {
    pub(super) fn new(device: &wgpu::Device, grid: [u32; 3]) -> Self {
        let make = |label| froxel_grid(device, grid, label).create_view(&Default::default());
        Self {
            grid,
            scatter: [make("Volumetric Scatter A"), make("Volumetric Scatter B")],
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Volumetric Fog Params"),
                size: std::mem::size_of::<super::VolumetricParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            current: 0,
            index: 0,
            history: None,
        }
    }

    /// Are these the targets for `grid`?
    pub(super) fn matches(&self, grid: [u32; 3]) -> bool {
        self.grid == grid
    }

    /// Start a frame under `key`: flip the grids and say what to reproject from,
    /// `None` after a cut or with no history.
    pub(super) fn advance(&mut self, key: HistoryKey) -> Option<Built> {
        let prev = self.history.filter(|h| h.key == key).map(|h| h.built);
        self.history = None;
        self.current = 1 - self.current;
        self.index = (self.index + 1) % PERIOD;
        prev
    }
}
