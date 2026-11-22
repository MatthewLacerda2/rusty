mod admit;
mod frame;
mod gpu;
mod pack;
pub(crate) mod slots;
pub(crate) mod uniform;

use std::collections::HashMap;

use glam::Vec3;

use self::admit::admitted;
pub(crate) use self::pack::pack_blend;
use self::slots::SlotTable;
pub(crate) use self::slots::{MAX_REFLECTION_PROBES, SPARE_SLOTS};
pub(crate) use self::uniform::ReflectionProbeUniform;
use super::cubemap::{CubeShape, CubemapTexture, CUBE_FORMAT};
use rusty_scene::{PreparedProbes, ReflectionProbe};

pub(crate) const MAX_FACE: u32 = *ReflectionProbe::RESOLUTION.end();

type Resident<'a> = (&'a ReflectionProbe, CubeShape, u32);

pub(super) type CubeCopy = (String, CubeShape, usize, u32);

pub(crate) struct ProbeArray {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    face: u32,
    mips: u32,
    cubes: usize,
    format: wgpu::TextureFormat,
    table: SlotTable,
    shapes: HashMap<String, Option<CubeShape>>,
    active: PreparedProbes,
    uniforms: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
    #[cfg(test)]
    pub(crate) copies: usize,
    #[cfg(test)]
    pub(crate) loads: usize,
}

impl ProbeArray {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let (texture, view) = gpu::stand_in(device);
        let format = match device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        {
            true => CUBE_FORMAT,
            false => wgpu::TextureFormat::Rgba16Float,
        };
        Self {
            texture,
            view,
            face: 1,
            mips: 1,
            cubes: 1,
            format,
            table: SlotTable::default(),
            shapes: HashMap::new(),
            active: PreparedProbes::default(),
            uniforms: [ReflectionProbeUniform::default(); MAX_REFLECTION_PROBES],
            #[cfg(test)]
            copies: 0,
            #[cfg(test)]
            loads: 0,
        }
    }

    pub(crate) fn probe_for(&self, centre: Vec3) -> u32 {
        pack_blend(self.active.blend(centre))
    }

    pub(crate) fn resident(&self, probe: &ReflectionProbe) -> Option<usize> {
        self.active.probes().iter().position(|r| r == probe)
    }

    pub(crate) fn uniforms(&self) -> &[ReflectionProbeUniform; MAX_REFLECTION_PROBES] {
        &self.uniforms
    }

    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    #[cfg(test)]
    pub(crate) fn force_uncompressed(&mut self) {
        self.format = wgpu::TextureFormat::Rgba16Float;
    }

    #[cfg(test)]
    pub(crate) fn face(&self) -> u32 {
        self.face
    }

    pub(crate) fn forget(&mut self, stale: impl Fn(&str) -> bool) {
        self.table.forget(&stale);
        self.shapes.retain(|p, _| !stale(p));
    }

    fn place(
        &mut self,
        device: &wgpu::Device,
        wanted: &[(&ReflectionProbe, CubeShape)],
        budget: u64,
    ) -> (Vec<CubeCopy>, bool) {
        let kept = admitted(wanted);
        if kept.is_empty() {
            self.activate(&[], &[]);
            return (Vec::new(), self.release_if_empty(device));
        }
        let (face, mips) = self.target_size(&kept);
        let kept: Vec<Resident> = kept
            .into_iter()
            .map(|(p, c)| (p, c, (face / c.size).trailing_zeros()))
            .collect();
        if face != self.face || mips != self.mips {
            self.table.clear();
        }
        let unique = distinct_cubes(&kept);
        let paths: Vec<&str> = unique
            .iter()
            .map(|(p, ..)| p.cubemap_path.as_str())
            .collect();
        let room = budget / frame::array_bytes(face, 1, self.format).max(1);
        let plan = self.table.plan(&paths, room as usize);
        let rebuilt = plan.capacity != self.cubes || face != self.face || mips != self.mips;
        if rebuilt {
            self.cubes = plan.capacity;
            (self.texture, self.view) =
                gpu::array_texture(device, face, mips, self.cubes, self.format);
            (self.face, self.mips) = (face, mips);
        }
        let copies = (plan.copies.iter())
            .map(|&slot| {
                let k = plan.slot_of.iter().position(|&s| s == slot).unwrap();
                let (_, shape, shift) = unique[k];
                (paths[k].to_string(), shape, slot, shift)
            })
            .collect();
        let layers: Vec<u32> = kept
            .iter()
            .map(|r| {
                plan.slot_of[paths.iter().position(|p| *p == r.0.cubemap_path).unwrap()] as u32
            })
            .collect();
        self.activate(&kept, &layers);
        (copies, rebuilt)
    }

    fn fill(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cubes: &[(CubemapTexture, usize, u32)],
    ) {
        gpu::copy_into(device, queue, &self.texture, cubes);
        #[cfg(test)]
        {
            self.copies += cubes.len();
        }
    }

    fn target_size(&self, kept: &[(&ReflectionProbe, CubeShape)]) -> (u32, u32) {
        let face = kept.iter().map(|(_, cube)| cube.size).max().unwrap_or(1);
        let mips = kept
            .iter()
            .map(|(_, cube)| (face / cube.size).trailing_zeros() + cube.mips)
            .max()
            .unwrap_or(1);
        if self.face == face && mips <= self.mips {
            return (face, self.mips);
        }
        (face, mips)
    }

    fn activate(&mut self, kept: &[Resident], layers: &[u32]) {
        self.active.update(kept.iter().map(|r| r.0));
        for (i, (&(probe, cube, shift), &layer)) in kept.iter().zip(layers).enumerate() {
            self.uniforms[i] = ReflectionProbeUniform::of(probe, shift, cube.mips, layer);
        }
        self.uniforms[kept.len()..].fill(ReflectionProbeUniform::default());
    }
}

fn distinct_cubes<'a>(kept: &[Resident<'a>]) -> Vec<Resident<'a>> {
    let mut unique: Vec<Resident> = Vec::new();
    for r in kept {
        if !unique.iter().any(|u| u.0.cubemap_path == r.0.cubemap_path) {
            unique.push(*r);
        }
    }
    unique
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
pub(crate) mod gpu_tests;
