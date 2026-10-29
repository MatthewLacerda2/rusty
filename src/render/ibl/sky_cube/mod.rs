//! The sky's reflection cube (#1553): what a surface outside every reflection probe
//! reflects. As in Unity, where the skybox *is* the default reflection probe
//! (Lighting → Environment → Environment Reflections), the sky is captured into a
//! cube and GGX-prefiltered into a roughness mip chain (`sky/prefilter.wgsl`), and
//! the forward pass samples it at `roughness * (mips - 1)` exactly as it samples a
//! probe (`forward::lighting::environment_reflection`). So a rough metal reflects a
//! blurred sky and a mirror a sharp one; the sky pass still draws the sky sharp.
//!
//! Two cubes, bound at group-0 binding 2 in the panorama's old place:
//! - **the panorama's**, baked when the scene's panorama texture changes (a load, a
//!   switch, a re-import), never for an unchanged one. The cube of the last
//!   panorama is kept while the procedural sky shows, so two scenes that alternate
//!   in one frame (the Inspector preview beside the viewport) bake nothing;
//! - **the procedural sky's**, baked once per renderer, on the first frame without
//!   a panorama. `procedural_sky` is affine in its tint, so the cube holds its two
//!   terms and the shader applies the ambient colour per sample: a tint that changes
//!   every frame (a scripted day cycle) never rebakes it, so no refresh needs
//!   Unity's realtime time slicing.
//!
//! GPU state rebuilt from the scene, never saved. Each bake is one compute pass,
//! timed as `Debug.Stats().gpu_passes.sky_reflection` and counted as
//! `sky_reflection_bakes`.

mod gpu;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::timing::GpuTimer;
use crate::render::GpuTexture;

/// The cubes' face size: a reflection probe's bake resolution, Unity's default
/// reflection resolution.
pub(crate) const SKY_FACE: u32 = crate::render::ibl::reflection_bake::DEFAULT_REFLECTION_RESOLUTION;

/// What a frame must bake, from what the cubes hold and the sky it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bake {
    Nothing,
    Procedural,
    Panorama,
}

/// The bake a frame needs: the procedural cube the first time no panorama shows,
/// the panorama's when the shown panorama is not the one its cube holds
/// (`held_matches`: `None` with no panorama shown).
pub(crate) fn plan(procedural_baked: bool, held_matches: Option<bool>) -> Bake {
    match held_matches {
        None if !procedural_baked => Bake::Procedural,
        Some(false) => Bake::Panorama,
        _ => Bake::Nothing,
    }
}

/// The two sky cubes, the bake pipeline and the sampler the forward pass reads them
/// through.
pub(crate) struct SkyReflection {
    filter: gpu::Filter,
    procedural: gpu::Cube,
    procedural_baked: bool,
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
            procedural: gpu::Cube::new(device, "Procedural Sky Reflection"),
            procedural_baked: false,
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
            _ => &self.procedural.view,
        }
    }

    /// Bake what showing `panorama` (`None`: the procedural sky) needs this frame,
    /// timing it on `timer`.
    pub(crate) fn prepare(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        timer: &GpuTimer,
        panorama: Option<&GpuTexture>,
    ) -> Prepared {
        let held = (self.panorama.as_ref(), panorama);
        let held_matches = match held {
            (_, None) => None,
            (Some((held, _)), Some(p)) => Some(*held == p.texture),
            (None, Some(_)) => Some(false),
        };
        match (plan(self.procedural_baked, held_matches), panorama) {
            (Bake::Procedural, _) => {
                self.filter
                    .bake((device, queue), timer, &self.procedural, None);
                self.procedural_baked = true;
                Prepared {
                    baked: true,
                    rebind: false,
                }
            }
            (Bake::Panorama, Some(p)) => {
                let cube = gpu::Cube::new(device, "Panorama Sky Reflection");
                self.filter.bake((device, queue), timer, &cube, Some(p));
                self.panorama = Some((p.texture.clone(), cube));
                Prepared {
                    baked: true,
                    rebind: true,
                }
            }
            _ => Prepared::default(),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod gpu_tests;
