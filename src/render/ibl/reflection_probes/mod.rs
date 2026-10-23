//! The reflection probes the forward pass shades with (#1206): the baked cubes of
//! the probes nearest the camera, copied into one cube-array texture at group-0
//! binding 4, and the probes themselves in `LightingUniforms.refl_probes`.
//!
//! Like Unity, a probe is picked per drawn object, not per camera or per pixel: the
//! draw's instance carries the index of the probe whose box contains its
//! world-bounds centre ([`ProbeArray::probe_for`]), and every fragment of it
//! box-projects against that probe and samples its cube's layer. Only probes with a
//! loaded cube are resident. A layer holds one cube *path*: probes that share a
//! cube share its layer, each keeping its own box and centre.
//!
//! **Per-frame cost.** Ordering the scene's probes by distance to the camera is
//! O(P log P) over P probes, then each draw tests at most [`MAX_REFLECTION_PROBES`]
//! boxes. GPU work happens only when the set of nearest cubes changes: one
//! texture-to-texture copy per newly resident cube (about 0.5 MB at the bake's
//! 128-texel face), or a rebuild of the array when it cannot hold them. A scene with
//! no baked probe (the Inspector preview, rendered every frame beside the viewport)
//! leaves the resident cubes alone, so the next scene finds them still in place.
//!
//! Cubes of different sizes share the array when each is the array's face size
//! halved some number of times: a smaller cube lands that many mips down, and its
//! `mip_base` tells the shader where its chain starts. A cube of any other size is
//! left out with a warning, and the objects it would cover reflect the sky.

mod frame;
mod gpu;
pub(crate) mod slots;

use std::rc::Rc;

use glam::Vec3;

use self::slots::SlotTable;
pub(crate) use self::slots::MAX_REFLECTION_PROBES;
use super::cubemap::CubemapTexture;
use crate::scene::{ReflectionProbe, ReflectionProbeSet};

/// One resident probe as the shader reads it: its capture point, its parallax box,
/// its cube's array layer and where the cube's mip chain sits in it. Mirrors
/// `ReflectionProbe` in `engine/shaders/lighting/reflection_probe.wgsl` byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ReflectionProbeUniform {
    pub center: [f32; 3],
    /// The array mip holding this cube's base level.
    pub mip_base: f32,
    pub box_min: [f32; 3],
    /// How many mips its chain has.
    pub mip_count: f32,
    pub box_max: [f32; 3],
    /// The array layer (cube) its cube was copied into.
    pub layer: u32,
}

/// A probe the array holds this frame: the probe, its cube, and how many mips below
/// the array's base level the cube's chain starts.
type Resident<'a> = (&'a ReflectionProbe, &'a CubemapTexture, u32);

/// The probe cube array and this frame's probes.
pub(crate) struct ProbeArray {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// The array's face size and mip count, and how many cubes it holds.
    face: u32,
    mips: u32,
    cubes: usize,
    table: SlotTable,
    /// This frame's resident probes in selection form; probe `i` is uniform `i`.
    active: ReflectionProbeSet,
    uniforms: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
    /// Cube paths already warned about for not fitting the array.
    warned: Vec<String>,
    /// How many cubes were copied in, for tests that a frame did no GPU work.
    #[cfg(test)]
    pub(crate) copies: usize,
}

impl ProbeArray {
    /// One black 1×1 cube: the array before any probe is resident.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let (texture, view) = gpu::array_texture(device, 1, 1, 1);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Reflection Probe Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            texture,
            view,
            sampler,
            face: 1,
            mips: 1,
            cubes: 1,
            table: SlotTable::default(),
            active: ReflectionProbeSet::new(),
            uniforms: [ReflectionProbeUniform::default(); MAX_REFLECTION_PROBES],
            warned: Vec::new(),
            #[cfg(test)]
            copies: 0,
        }
    }

    /// The resident probe plus one for an object whose world bounds centre on
    /// `centre`, or 0 when no resident probe's box contains it (it reflects the sky).
    pub(crate) fn probe_for(&self, centre: Vec3) -> u32 {
        self.active.select(centre).map_or(0, |(i, _)| i as u32 + 1)
    }

    /// Every resident probe, as `LightingUniforms.refl_probes` holds them.
    pub(crate) fn uniforms(&self) -> &[ReflectionProbeUniform; MAX_REFLECTION_PROBES] {
        &self.uniforms
    }

    /// The array's face size: the largest resident cube's.
    #[cfg(test)]
    pub(crate) fn face(&self) -> u32 {
        self.face
    }

    /// Drop the slots of re-written cubemaps (#1207); the next frame copies them again.
    pub(crate) fn forget(&mut self, stale: impl Fn(&str) -> bool) {
        self.table.forget(stale);
    }

    /// Make `wanted` (nearest first, at most the budget) this frame's probes: copy the
    /// cubes that are not resident yet, rebuilding the array when it cannot hold them.
    /// Returns true when the array texture was replaced, so group 0 must rebind.
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wanted: &[(&ReflectionProbe, Rc<CubemapTexture>)],
    ) -> bool {
        let kept = self.fitting(wanted);
        if kept.is_empty() {
            // Nothing to shade: keep the resident cubes for the next scene.
            self.activate(&[], &[]);
            return false;
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
        let plan = self.table.plan(&paths);
        let rebuilt = plan.capacity != self.cubes || face != self.face || mips != self.mips;
        if rebuilt {
            self.cubes = plan.capacity;
            (self.texture, self.view) = gpu::array_texture(device, face, mips, self.cubes);
            (self.face, self.mips) = (face, mips);
        }
        let copies: Vec<_> = plan
            .copies
            .iter()
            .map(|&slot| {
                (
                    unique[plan.slot_of.iter().position(|&s| s == slot).unwrap()],
                    slot,
                )
            })
            .collect();
        gpu::copy_into(device, queue, &self.texture, &copies);
        #[cfg(test)]
        {
            self.copies += copies.len();
        }
        let layers: Vec<u32> = kept
            .iter()
            .map(|r| {
                plan.slot_of[paths.iter().position(|p| *p == r.0.cubemap_path).unwrap()] as u32
            })
            .collect();
        self.activate(&kept, &layers);
        rebuilt
    }

    /// The wanted probes whose cube fits an array sized to the largest of them (its
    /// face halved a whole number of times); each other cube is warned about once.
    fn fitting<'a>(
        &mut self,
        wanted: &'a [(&'a ReflectionProbe, Rc<CubemapTexture>)],
    ) -> Vec<(&'a ReflectionProbe, &'a CubemapTexture)> {
        let face = wanted.iter().map(|(_, c)| c.size).max().unwrap_or(1);
        let mut kept = Vec::new();
        for (probe, cube) in wanted {
            if fits(face, cube.size) {
                kept.push((*probe, &**cube));
            } else if !self.warned.contains(&probe.cubemap_path) {
                log::warn!(
                    "[Reflection] cubemap {} is {} texels, not {face} halved a whole number \
                     of times; its objects reflect the sky",
                    probe.cubemap_path,
                    cube.size
                );
                self.warned.push(probe.cubemap_path.clone());
            }
        }
        kept
    }

    /// The face size and mip count to hold `kept`: the current array's when it can
    /// hold every cube (so a scene with smaller cubes does not reallocate), else the
    /// set's own.
    fn target_size(&self, kept: &[(&ReflectionProbe, &CubemapTexture)]) -> (u32, u32) {
        let needs = |face: u32| {
            let mut mips = 0;
            for (_, cube) in kept {
                if !fits(face, cube.size) {
                    return None;
                }
                let shift = (face / cube.size).trailing_zeros();
                mips = mips.max(shift + cube.texture.mip_level_count());
            }
            Some(mips)
        };
        if needs(self.face).is_some_and(|mips| mips <= self.mips) {
            return (self.face, self.mips);
        }
        let face = kept.iter().map(|(_, c)| c.size).max().unwrap_or(1);
        (face, needs(face).unwrap_or(1))
    }

    /// Record `kept` as the frame's probes, probe `i` in layer `layers[i]`: the
    /// selection set draws query and the uniforms the shader reads. Only a probe that
    /// changed since the last frame is cloned.
    fn activate(&mut self, kept: &[Resident], layers: &[u32]) {
        self.active.probes.truncate(kept.len());
        for (i, (&(probe, cube, shift), &layer)) in kept.iter().zip(layers).enumerate() {
            self.uniforms[i] = ReflectionProbeUniform {
                center: probe.position.to_array(),
                mip_base: shift as f32,
                box_min: probe.box_min.to_array(),
                mip_count: cube.texture.mip_level_count() as f32,
                box_max: probe.box_max.to_array(),
                layer,
            };
            match self.active.probes.get_mut(i) {
                Some(resident) if resident == probe => {}
                Some(resident) => *resident = probe.clone(),
                None => self.active.probes.push(probe.clone()),
            }
        }
    }
}

/// Whether a `size`-texel cube fits an array of `face` texels: `face` halved a whole
/// number of times.
fn fits(face: u32, size: u32) -> bool {
    face.is_multiple_of(size) && (face / size).is_power_of_two()
}

/// One entry per distinct cube path in `kept`, in first-wanted order: the cubes that
/// take an array layer.
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
mod gpu_tests;
