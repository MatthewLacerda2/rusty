//! The light probes in the air (#1703): the ambient the froxels scatter, taken
//! from the scene's baked probe field instead of the one scene-wide ambient, so a
//! sealed room's air is as dark as its probes and a lit hall's as bright as its.
//!
//! A coarse grid of [`GRID`] cells around the camera, each the probe field's L0
//! (its average radiance over every direction, which is exactly what an isotropic
//! medium scatters) at the cell's centre, sampled as a Light Probe Proxy Volume's
//! points are (`ProbeVolume::sample`, `render::draw::proxy_volume`). Its cells are
//! `2 × depth_extent / 16` wide, so it spans the froxels' reach across and half of
//! it up; it is snapped to whole cells, so it does not swim as the camera moves.
//! It is filled on the CPU only when the probes change, the camera crosses into
//! another cell or the extent changes (16 × 8 × 16 = 2048 samples, a 16 KB
//! upload); every other frame it costs nothing. Beyond it the froxels read its
//! nearest face. With no probes the froxels take the scene ambient as before.

use glam::{IVec3, Vec3};

use super::volume::froxel_grid;
use rusty_core::color::f32_to_f16;
use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

/// Cells across, up and deep.
pub(crate) const GRID: [u32; 3] = [16, 8, 16];

/// SH's constant basis: an L0 coefficient times it is the average radiance.
const Y00: f32 = 0.282_094_8;

/// What the grid was filled under.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    scene: SceneId,
    probes: Revision,
    origin: IVec3,
    cell: u32,
}

pub(crate) struct AirProbes {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    key: Option<Key>,
    /// How many times the grid was filled: what a test counts a kept frame by.
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

    /// The grid for a camera at `eye` reaching `extent` metres, filled when it
    /// changed. Returns the uniform's two rows: the box's corner (w 1 when the
    /// scene has probes, 0 when the froxels take the scene ambient) and its size.
    pub(crate) fn refresh(
        &mut self,
        scene: &Scene,
        (eye, extent): (Vec3, f32),
        queue: &wgpu::Queue,
    ) -> [[f32; 4]; 2] {
        if scene.probes.is_empty() {
            self.key = None;
            return [[0.0; 4]; 2];
        }
        let cell = 2.0 * extent / GRID[0] as f32;
        let dims = IVec3::from_array(GRID.map(|n| n as i32));
        let origin = (eye / cell).floor().as_ivec3() - dims / 2;
        let key = Key {
            scene: scene.id(),
            probes: scene.probes.revision(),
            origin,
            cell: cell.to_bits(),
        };
        let min = origin.as_vec3() * cell;
        if self.key != Some(key) {
            self.fill(scene, (min, cell), queue);
            self.key = Some(key);
            self.fills += 1;
        }
        let size = dims.as_vec3() * cell;
        [min.extend(1.0).to_array(), size.extend(0.0).to_array()]
    }

    /// Sample the probe field at every cell's centre and upload it.
    fn fill(&self, scene: &Scene, (min, cell): (Vec3, f32), queue: &wgpu::Queue) {
        let [w, h, d] = GRID;
        let mut texels = Vec::with_capacity((w * h * d * 4) as usize);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let at = min + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * cell;
                    let l0 = scene.probes.sample(at).map_or([0.0; 3], |sh| sh.coeffs[0]);
                    let rgb = l0.map(|c| (c * Y00).max(0.0));
                    texels.extend(rgb.map(f32_to_f16));
                    texels.push(f32_to_f16(1.0));
                }
            }
        }
        queue.write_texture(
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
