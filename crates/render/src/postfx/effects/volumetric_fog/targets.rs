use super::params::Built;
use super::volume::froxel_grid;
use crate::motion::HistoryKey;

#[derive(Clone, Copy, Debug)]
pub(crate) struct History {
    pub key: HistoryKey,
    pub built: Built,
}

pub(crate) struct VolumetricTargets {
    grid: [u32; 3],
    pub(super) scatter: [wgpu::TextureView; 2],
    pub(super) uniform: wgpu::Buffer,
    pub(super) current: usize,
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

    pub(super) fn matches(&self, grid: [u32; 3]) -> bool {
        self.grid == grid
    }

    pub(super) fn advance(&mut self, key: HistoryKey) -> Option<Built> {
        let kept = self.history.filter(|h| key.continues(&h.key));
        let prev = kept.map(|h| h.built.shifted(key.moved_since(&h.key)));
        self.history = None;
        self.current = 1 - self.current;
        self.index = (self.index + 1) % PERIOD;
        prev
    }
}
