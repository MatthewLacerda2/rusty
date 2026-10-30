//! The reflection probes the forward pass shades with (#1206): the baked cubes of
//! the probes whose boxes are nearest the camera (#1238), copied into one cube-array
//! texture at group-0 binding 4, and the probes themselves in
//! `LightingUniforms.refl_probes`.
//!
//! Like Unity's *Blend Probes*, probes are picked per drawn object, not per camera
//! or per pixel (#1584): the draw's instance carries the two resident probes its
//! world-bounds centre blends and their weights ([`ProbeArray::probe_for`], packed by
//! [`pack_blend`]), and every fragment of it samples both, box-projected when the
//! probe asks for it, and the sky for what they leave. Only probes with a loaded
//! cube are resident. A layer holds one cube *path*: probes that share a cube share
//! its layer, each keeping its own box and centre.
//!
//! **Per-frame cost.** Ordering the scene's probes by the camera's distance to their
//! boxes is O(P log P) over P probes, then each draw tests at most
//! [`MAX_REFLECTION_PROBES`] boxes. GPU work happens only when the set of nearest
//! cubes changes: one texture-to-texture copy per newly resident cube, or a rebuild
//! of the array when it cannot hold them. A scene with no baked probe (the Inspector
//! preview, rendered every frame beside the viewport) leaves the resident cubes
//! alone, so the next scene finds them still in place.
//!
//! **Size and budget.** The array's face is the largest resident cube's (#1584); a
//! smaller cube lands as many mips down as it is halved, and its `mip_base` tells the
//! shader where its chain starts, so a 128-texel cube shares an array of 512-texel
//! ones. The array is the only copy of a resident cube: a cube is read from disk,
//! copied into its slot, and its own texture dropped, so the scene's byte budget
//! (`LightingSettings::reflections`, counted at the array's face) bounds all the
//! probe memory there is. The frame fills it nearest box first, judging each cube by
//! its KTX2 header before reading the rest of it, and leaves room in it for
//! [`SPARE_SLOTS`] cubes that just left range (`frame.rs`, `slots.rs`). A cube
//! whose size is not a power of two up to [`MAX_FACE`] is left out with a warning,
//! never uploaded (#1245), and the objects it would cover reflect the sky.

mod admit;
mod frame;
mod gpu;
mod pack;
pub(crate) mod slots;

use std::collections::HashMap;

use glam::Vec3;

use self::admit::admitted;
pub(crate) use self::pack::pack_blend;
use self::slots::SlotTable;
pub(crate) use self::slots::{MAX_REFLECTION_PROBES, SPARE_SLOTS};
use super::cubemap::{CubeShape, CubemapTexture};
use crate::scene::{ReflectionProbe, ReflectionProbeSet};

/// The largest face a resident cube may have: the largest a probe bakes at.
pub(crate) const MAX_FACE: u32 = *ReflectionProbe::RESOLUTION.end();

/// One resident probe as the shader reads it: its capture point, its parallax box,
/// its cube's array layer, where the cube's mip chain sits in it, and its intensity
/// and box-projection switch. Mirrors
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
    /// `x` intensity, `y` 1 when box projection is on.
    pub params: [f32; 4],
}

/// A probe the array holds this frame: the probe, its cube's shape, and how many mips
/// below the array's base level the cube's chain starts.
type Resident<'a> = (&'a ReflectionProbe, CubeShape, u32);

/// A cube the array must read from disk and copy in this frame: its path and shape,
/// its slot, and how many mips down it lands.
pub(super) type CubeCopy = (String, CubeShape, usize, u32);

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
    /// The shape of every cube path the frame has looked at, from its KTX2 header;
    /// `None` for a file that is missing or that the array does not admit.
    shapes: HashMap<String, Option<CubeShape>>,
    /// This frame's resident probes in selection form; probe `i` is uniform `i`.
    active: ReflectionProbeSet,
    uniforms: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
    /// How many cubes were copied in, for tests that a frame did no GPU work.
    #[cfg(test)]
    pub(crate) copies: usize,
    /// How many cubes were read from disk, for tests of what a frame loads.
    #[cfg(test)]
    pub(crate) loads: usize,
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
            shapes: HashMap::new(),
            active: ReflectionProbeSet::new(),
            uniforms: [ReflectionProbeUniform::default(); MAX_REFLECTION_PROBES],
            #[cfg(test)]
            copies: 0,
            #[cfg(test)]
            loads: 0,
        }
    }

    /// The packed blend ([`pack_blend`]) of the resident probes an object whose world
    /// bounds centre on `centre` reflects; 0 (the sky) when none reaches it.
    pub(crate) fn probe_for(&self, centre: Vec3) -> u32 {
        pack_blend(self.active.blend(centre))
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

    /// Drop the slots of re-written cubemaps (#1207), and their shapes; the next
    /// frame reads and copies them again.
    pub(crate) fn forget(&mut self, stale: impl Fn(&str) -> bool) {
        self.table.forget(&stale);
        self.shapes.retain(|p, _| !stale(p));
    }

    /// Make `wanted` (in scene order, within the budget) this frame's probes: place
    /// them in slots, resizing the array to the slots they and the spares need (no
    /// more than `budget` bytes pays for, past what they need), and return the cubes
    /// that must be read and copied in ([`Self::fill`]) and whether the array texture
    /// was replaced, so group 0 must rebind. Each cube must be
    /// [admitted](Self::admits): a debug build asserts it, and a release build leaves
    /// out one that isn't, since a cube larger than the face would give the array a
    /// mip count wgpu rejects (#1245).
    fn place(
        &mut self,
        device: &wgpu::Device,
        wanted: &[(&ReflectionProbe, CubeShape)],
        budget: u64,
    ) -> (Vec<CubeCopy>, bool) {
        let kept = admitted(wanted);
        if kept.is_empty() {
            // Nothing to shade: keep the resident cubes for the next scene.
            self.activate(&[], &[]);
            return (Vec::new(), false);
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
        let room = budget / frame::array_bytes(face, 1).max(1);
        let plan = self.table.plan(&paths, room as usize);
        let rebuilt = plan.capacity != self.cubes || face != self.face || mips != self.mips;
        if rebuilt {
            self.cubes = plan.capacity;
            (self.texture, self.view) = gpu::array_texture(device, face, mips, self.cubes);
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

    /// Copy each loaded `(cube, slot, shift)` [`Self::place`] asked for into its
    /// slot. The cubes are dropped after: the array is the copy that stays.
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

    /// The face size and mip count to hold `kept`: the largest cube's face, and the
    /// current array's mips when they hold every cube's chain (so a scene with smaller
    /// cubes does not reallocate), else as many as the set needs.
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

    /// Record `kept` as the frame's probes, probe `i` in layer `layers[i]`: the
    /// selection set draws query and the uniforms the shader reads. Only a probe that
    /// changed since the last frame is cloned; a loaded scene's probes were held to
    /// their ranges once, on load (#1584).
    fn activate(&mut self, kept: &[Resident], layers: &[u32]) {
        self.active.probes.truncate(kept.len());
        for (i, (&(probe, cube, shift), &layer)) in kept.iter().zip(layers).enumerate() {
            match self.active.probes.get_mut(i) {
                Some(resident) if resident == probe => {}
                Some(resident) => resident.clone_from(probe),
                None => self.active.probes.push(probe.clone()),
            }
            let probe = &self.active.probes[i];
            self.uniforms[i] = ReflectionProbeUniform {
                center: probe.position.to_array(),
                mip_base: shift as f32,
                box_min: probe.box_min.to_array(),
                mip_count: cube.mips as f32,
                box_max: probe.box_max.to_array(),
                layer,
                params: [
                    probe.intensity,
                    f32::from(u8::from(probe.box_projection)),
                    0.0,
                    0.0,
                ],
            };
        }
    }
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
