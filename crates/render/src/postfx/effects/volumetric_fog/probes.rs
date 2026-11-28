//! The grid holds the probe field's L0 (its average radiance, which is what an
//! isotropic medium scatters), cells `2 * depth_extent / 16` wide, snapped to
//! whole cells so it does not swim. Beyond it the froxels read its nearest face.

use crate::gpu::ops::{CountedQueue as _, Writes};
use glam::{IVec3, Vec3};

use super::volume::froxel_grid;
use rusty_core::color::f32_to_f16;
use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

pub(crate) const GRID: [u32; 3] = [16, 8, 16];

/// SH's constant basis: an L0 coefficient times it is the average radiance.
const Y00: f32 = 0.282_094_8;

#[derive(Clone, PartialEq)]
struct Key {
    scene: SceneId,
    probes: Vec<(u32, Revision)>,
    origin: IVec3,
    cell: u32,
}

pub(crate) struct AirProbes {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    key: Option<Key>,
    pub(crate) fills: u64,
}

impl AirProbes {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = froxel_grid(device, GRID, "Volumetric Fog Probes");
        Self {
            view: texture.create_view(&Default::default()),
            texture,
            key: None,
            fills: 0,
        }
    }

    /// Returns the uniform's two rows: the box's corner (w 1 when the scene has
    /// probes, 0 when the froxels take the scene ambient) and its size.
    pub(crate) fn refresh(
        &mut self,
        scene: &Scene,
        (eye, extent): (Vec3, f32),
        writes: Writes,
    ) -> [[f32; 4]; 2] {
        if scene.probes_empty() {
            self.key = None;
            return [[0.0; 4]; 2];
        }
        let cell = 2.0 * extent / GRID[0] as f32;
        let dims = IVec3::from_array(GRID.map(|n| n as i32));
        let origin = (eye / cell).floor().as_ivec3() - dims / 2;
        let key = Key {
            scene: scene.id(),
            probes: scene.with_probe_field(|f| f.revisions()),
            origin,
            cell: cell.to_bits(),
        };
        let min = origin.as_vec3() * cell;
        if self.key.as_ref() != Some(&key) {
            self.fill(scene, (min, cell), writes);
            self.key = Some(key);
            self.fills += 1;
        }
        let size = dims.as_vec3() * cell;
        [min.extend(1.0).to_array(), size.extend(0.0).to_array()]
    }

    fn fill(&self, scene: &Scene, (min, cell): (Vec3, f32), (queue, staging): Writes) {
        scene.with_probe_field(|field| self.fill_from(field, (min, cell), (queue, staging)));
    }

    fn fill_from(
        &self,
        field: rusty_scene::lighting::probe::ProbeField<'_>,
        (min, cell): (Vec3, f32),
        (queue, staging): Writes,
    ) {
        let [w, h, d] = GRID;
        let mut texels = Vec::with_capacity((w * h * d * 4) as usize);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let at = min + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * cell;
                    let l0 = field.sample(at).map_or([0.0; 3], |sh| sh.coeffs[0]);
                    let rgb = l0.map(|c| (c * Y00).max(0.0));
                    texels.extend(rgb.map(f32_to_f16));
                    texels.push(f32_to_f16(1.0));
                }
            }
        }
        queue.counted_write_texture(
            staging,
            self.texture.as_image_copy(),
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 8),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: d,
            },
        );
    }
}
