//! The sky's reflection cube (#1553): what a surface outside every reflection probe
//! reflects. As in Unity, where the skybox *is* the default reflection probe
//! (Lighting → Environment → Environment Reflections), the sky is captured into a
//! cube and GGX-prefiltered into a roughness mip chain (`sky/prefilter.wgsl`), and
//! the forward pass samples it at `roughness * (mips - 1)` exactly as it samples a
//! probe (`forward::lighting::environment_reflection`). So a rough metal reflects a
//! blurred sky and a mirror a sharp one; the sky pass still draws the sky sharp.
//!
//! Three cubes, one bound at group-0 binding 2 in the panorama's old place:
//! - **the panorama's**, baked when the scene's panorama texture changes (a load, a
//!   switch, a re-import), never for an unchanged one. The cube of the last
//!   panorama is kept while the procedural sky shows, so two scenes that alternate
//!   in one frame (the Inspector preview beside the viewport) bake nothing;
//! - **the procedural sky's two**, each holding the sky for one set of the scene's
//!   sky settings (#1620): baked on the first frame that shows settings neither
//!   holds, into the slot not bound, so two skies drawn in one frame (the Inspector
//!   preview's default beside a scene's own) both stay baked. For fixed settings
//!   `procedural_sky` is affine in the ambient colour, so a cube holds its two terms
//!   and the shader applies the ambient colour per sample: an ambient that changes
//!   every frame (a scripted day cycle) never rebakes it, so no refresh needs
//!   Unity's realtime time slicing; a settings change bakes once.
//!
//! GPU state rebuilt from the scene, never saved. Each bake is one compute pass,
//! timed as `Debug.Stats().gpu_passes.sky_reflection` and counted as
//! `sky_reflection_bakes`.

mod gpu;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::uniforms::ProceduralSkyUniform;
use crate::render::timing::GpuTimer;
use crate::render::GpuTexture;

/// The cubes' face size: Unity's default environment-reflection resolution.
pub(crate) const SKY_FACE: u32 = 128;

/// What a frame must bake, from what the cubes hold and the sky it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bake {
    Nothing,
    Procedural,
    Panorama,
}

/// The bake a frame needs: a procedural cube when no panorama shows and no cube
/// holds the shown sky settings (`procedural_held`), the panorama's when the shown
/// panorama is not the one its cube holds (`held_matches`: `None` with no panorama
/// shown).
pub(crate) fn plan(procedural_held: bool, held_matches: Option<bool>) -> Bake {
    match held_matches {
        None if !procedural_held => Bake::Procedural,
        Some(false) => Bake::Panorama,
        _ => Bake::Nothing,
    }
}

/// One procedural sky cube and the sky settings it was baked for (`None`: never).
struct Procedural {
    cube: gpu::Cube,
    holds: Option<ProceduralSkyUniform>,
}

/// The slot to bake `sky` into: the bound one while it holds nothing (the first
/// bake), else the other, so the sky bound last is kept.
fn bake_slot(current: usize, current_empty: bool) -> usize {
    if current_empty {
        current
    } else {
        1 - current
    }
}

/// The sky cubes, the bake pipeline and the sampler the forward pass reads them
/// through.
pub(crate) struct SkyReflection {
    filter: gpu::Filter,
    procedural: [Procedural; 2],
    /// The procedural slot [`Self::view`] binds.
    current: usize,
    /// The panorama texture the panorama cube was baked from, and the cube.
    panorama: Option<(wgpu::Texture, gpu::Cube)>,
    /// Trilinear, clamped: the roughness picks the mip.
    pub sampler: wgpu::Sampler,
}

/// What [`SkyReflection::prepare`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Prepared {
    pub baked: bool,
    /// The cube to bind was replaced: group 0 must be rebuilt.
    pub rebind: bool,
}

impl SkyReflection {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sky Reflection Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            filter: gpu::Filter::new(device, registry),
            procedural: std::array::from_fn(|_| Procedural {
                cube: gpu::Cube::new(device, "Procedural Sky Reflection"),
                holds: None,
            }),
            current: 0,
            panorama: None,
            sampler,
        }
    }

    /// The cube showing `panorama` (`None`: the procedural sky): what group 0 binds
    /// once [`Self::prepare`] ran for it. The shader tells the two apart by
    /// `LightingUniforms.sky_textured`.
    pub(crate) fn view(&self, panorama: Option<&GpuTexture>) -> &wgpu::TextureView {
        match (&self.panorama, panorama) {
            (Some((held, cube)), Some(p)) if *held == p.texture => &cube.view,
            _ => &self.procedural[self.current].cube.view,
        }
    }

    /// Bake what showing `panorama` (`None`: the procedural sky, shaped by `sky`)
    /// needs this frame, timing it on `timer`.
    pub(crate) fn prepare(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        timer: &GpuTimer,
        (panorama, sky): (Option<&GpuTexture>, &ProceduralSkyUniform),
    ) -> Prepared {
        let held = (self.panorama.as_ref(), panorama);
        let held_matches = match held {
            (_, None) => None,
            (Some((held, _)), Some(p)) => Some(*held == p.texture),
            (None, Some(_)) => Some(false),
        };
        let held = self.procedural.iter().position(|p| p.holds == Some(*sky));
        match (plan(held.is_some(), held_matches), panorama) {
            (Bake::Procedural, _) => {
                let empty = self.procedural[self.current].holds.is_none();
                let slot = bake_slot(self.current, empty);
                let target = &mut self.procedural[slot];
                self.filter
                    .bake((device, queue), timer, &target.cube, (None, sky));
                target.holds = Some(*sky);
                self.select(slot, true)
            }
            (Bake::Panorama, Some(p)) => {
                let cube = gpu::Cube::new(device, "Panorama Sky Reflection");
                self.filter
                    .bake((device, queue), timer, &cube, (Some(p), sky));
                self.panorama = Some((p.texture.clone(), cube));
                Prepared {
                    baked: true,
                    rebind: true,
                }
            }
            (_, None) => self.select(held.unwrap_or(self.current), false),
            _ => Prepared::default(),
        }
    }

    /// Bind procedural slot `slot` from now on: group 0 is rebuilt when it is not
    /// the one bound.
    fn select(&mut self, slot: usize, baked: bool) -> Prepared {
        let rebind = slot != self.current;
        self.current = slot;
        Prepared { baked, rebind }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod gpu_tests;

#[cfg(test)]
#[path = "settings_gpu_tests.rs"]
mod settings_gpu_tests;
