//! The reflection probes the forward pass shades with (#1206): the baked cubes of
//! the probes nearest the camera, copied into one cube-array texture at group-0
//! binding 4, and their boxes in `LightingUniforms.refl_probes`.
//!
//! Like Unity, a probe is picked per drawn object, not per camera or per pixel: the
//! draw's instance carries the slot of the probe whose box contains its world-bounds
//! centre ([`ProbeArray::slot_for`]), and every fragment of it box-projects against
//! that probe and samples its layer. Only probes with a loaded cube take a slot.
//!
//! **Per-frame cost.** Ordering the scene's probes by distance to the camera is
//! O(P log P) over P probes, then each draw tests at most [`MAX_REFLECTION_PROBES`]
//! boxes. GPU work happens only when the set of nearest probes changes: one
//! texture-to-texture copy per newly resident cube (about 0.5 MB at the bake's
//! 128-texel face), or a rebuild of the array when it must grow.
//!
//! Cubes of different sizes share the array when each is the array's face size
//! halved some number of times: a smaller cube lands that many mips down, and its
//! `mip_base` tells the shader where its chain starts. A cube of any other size is
//! left out, and the objects it would cover reflect the sky.

pub(crate) mod slots;

use std::rc::Rc;

use glam::Vec3;

pub(crate) use self::slots::MAX_REFLECTION_PROBES;
use self::slots::{SlotPlan, SlotTable};
use super::cubemap::CubemapTexture;
use crate::render::Renderer;
use crate::scene::{ReflectionProbe, ReflectionProbeSet, Scene};

/// One shaded probe as the shader reads it: its capture point, its parallax box and
/// where its mip chain sits in its array layer. Mirrors `ReflectionProbe` in
/// `common.wgsl` byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ReflectionProbeUniform {
    pub center: [f32; 3],
    /// The array mip holding this cube's base level.
    pub mip_base: f32,
    pub box_min: [f32; 3],
    /// How many mips its chain has.
    pub mip_count: f32,
    pub box_max: [f32; 3],
    pub _pad: f32,
}

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
    /// This frame's shaded probes in selection form, and the slot of each.
    active: ReflectionProbeSet,
    active_slot: Vec<u32>,
    uniforms: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
}

impl ProbeArray {
    /// One black 1×1 cube: the array before any probe is resident.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let (texture, view) = array_texture(device, 1, 1, 1);
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
            active_slot: Vec::new(),
            uniforms: [ReflectionProbeUniform::default(); MAX_REFLECTION_PROBES],
        }
    }

    /// The probe slot plus one for an object whose world bounds centre on `centre`,
    /// or 0 when no resident probe's box contains it (the object reflects the sky).
    pub(crate) fn slot_for(&self, centre: Vec3) -> u32 {
        self.active
            .select(centre)
            .map_or(0, |(i, _)| self.active_slot[i] + 1)
    }

    /// Every slot's probe, as `LightingUniforms.refl_probes` holds them.
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
    /// cubes that are not resident yet, growing the array when they do not fit.
    /// Returns true when the array texture was replaced, so group 0 must rebind.
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wanted: &[(&ReflectionProbe, Rc<CubemapTexture>)],
    ) -> bool {
        let face = wanted.iter().map(|(_, c)| c.size).max().unwrap_or(1);
        // Each fitting cube with the number of mips it lands below the array's base.
        let wanted: Vec<Resident> = wanted
            .iter()
            .filter(|(_, c)| face % c.size == 0 && (face / c.size).is_power_of_two())
            .map(|(p, c)| (*p, &**c, (face / c.size).trailing_zeros()))
            .collect();
        let mips = wanted
            .iter()
            .map(|(_, c, shift)| shift + c.texture.mip_level_count())
            .max()
            .unwrap_or(1);
        if face != self.face || mips != self.mips {
            self.table.clear();
        }
        let paths: Vec<&str> = wanted
            .iter()
            .map(|(p, ..)| p.cubemap_path.as_str())
            .collect();
        let plan = self.table.plan(&paths);
        let rebuilt = plan.capacity.max(1) != self.cubes || face != self.face || mips != self.mips;
        if rebuilt {
            self.cubes = plan.capacity.max(1);
            (self.texture, self.view) = array_texture(device, face, mips, self.cubes);
            (self.face, self.mips) = (face, mips);
        }
        self.copy_planned(device, queue, &wanted, &plan);
        self.activate(&wanted, &plan.slot_of);
        rebuilt
    }

    /// Copy the cubes `plan` names into their slots, in one submission.
    fn copy_planned(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wanted: &[Resident],
        plan: &SlotPlan,
    ) {
        if plan.copies.is_empty() {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Reflection Probe Copies"),
        });
        for &slot in &plan.copies {
            let k = plan.slot_of.iter().position(|&s| s == slot);
            let (_, cube, shift) = wanted[k.expect("a copied slot is planned")];
            copy_cube(&mut encoder, cube, &self.texture, slot as u32, shift);
        }
        queue.submit([encoder.finish()]);
    }

    /// Record `wanted` as the frame's probes in `slot_of`'s slots: the selection set
    /// draws query and the uniforms the shader reads.
    fn activate(&mut self, wanted: &[Resident], slot_of: &[usize]) {
        self.active.probes.clear();
        self.active_slot.clear();
        for (&(probe, cube, shift), &slot) in wanted.iter().zip(slot_of) {
            self.active.probes.push(probe.clone());
            self.active_slot.push(slot as u32);
            self.uniforms[slot] = ReflectionProbeUniform {
                center: probe.position.to_array(),
                mip_base: shift as f32,
                box_min: probe.box_min.to_array(),
                mip_count: cube.texture.mip_level_count() as f32,
                box_max: probe.box_max.to_array(),
                _pad: 0.0,
            };
        }
    }
}

/// A probe the array holds this frame: the probe, its cube, and how many mips below
/// the array's base level the cube's chain starts.
type Resident<'a> = (&'a ReflectionProbe, &'a CubemapTexture, u32);

/// A cube-array texture of `cubes` black cubes, `face` texels square with `mips`
/// levels, and its `CubeArray` view.
fn array_texture(
    device: &wgpu::Device,
    face: u32,
    mips: u32,
    cubes: usize,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Reflection Probe Array"),
        size: wgpu::Extent3d {
            width: face,
            height: face,
            depth_or_array_layers: 6 * cubes as u32,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Reflection Probe Array"),
        dimension: Some(wgpu::TextureViewDimension::CubeArray),
        ..Default::default()
    });
    (texture, view)
}

/// Record the copy of every mip of `cube` into array layer `slot`, `shift` mips down.
fn copy_cube(
    encoder: &mut wgpu::CommandEncoder,
    cube: &CubemapTexture,
    array: &wgpu::Texture,
    slot: u32,
    shift: u32,
) {
    for level in 0..cube.texture.mip_level_count() {
        let size = (cube.size >> level).max(1);
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &cube.texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: array,
                mip_level: level + shift,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: 6 * slot,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
        );
    }
}

impl Renderer {
    /// Make the baked probes nearest `camera_pos` this frame's resident probes
    /// (#1206), loading their cubes through the by-path cache (#355), and mark group 0
    /// for a rebind when the array texture was replaced. Ties in distance go to the
    /// lower probe index, so the set is the same every run.
    pub(crate) fn update_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) {
        let probes = &scene.reflection_probes.probes;
        let mut order: Vec<usize> = (0..probes.len())
            .filter(|&i| !probes[i].cubemap_path.is_empty())
            .collect();
        let distance = |i: usize| probes[i].position.distance_squared(camera_pos);
        order.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)).then(a.cmp(&b)));
        let mut wanted = Vec::new();
        for i in order {
            if wanted.len() == MAX_REFLECTION_PROBES {
                break;
            }
            if let Some(cube) = self.cached_cubemap(&probes[i].cubemap_path) {
                wanted.push((&probes[i], cube));
            }
        }
        if self
            .reflection_probes
            .update(&self.device, &self.queue, &wanted)
        {
            self.global_bind_group_dirty = true;
        }
    }
}
