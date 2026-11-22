mod gpu;
mod spare;

pub(crate) use spare::SpareGroup;

use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::uniforms::ProceduralSkyUniform;
use crate::timing::GpuTimer;
use crate::GpuTexture;

pub(crate) const SKY_FACE: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bake {
    Nothing,
    Procedural,
    Panorama,
}

pub(crate) fn plan(procedural_held: bool, held_matches: Option<bool>) -> Bake {
    match held_matches {
        None if !procedural_held => Bake::Procedural,
        Some(false) => Bake::Panorama,
        _ => Bake::Nothing,
    }
}

struct Procedural {
    cube: gpu::Cube,
    holds: Option<ProceduralSkyUniform>,
}

fn procedural_step<K: PartialEq>(
    holds: &[Option<K>; 2],
    current: usize,
    sky: &K,
) -> (usize, Prepared) {
    let (slot, baked) = match holds.iter().position(|h| h.as_ref() == Some(sky)) {
        Some(slot) => (slot, false),
        None => (bake_slot(current, holds[current].is_none()), true),
    };
    let switched = slot != current;
    (
        slot,
        Prepared {
            baked,
            rebind: false,
            switched,
        },
    )
}

fn bake_slot(current: usize, current_empty: bool) -> usize {
    if current_empty {
        current
    } else {
        1 - current
    }
}

pub(crate) struct SkyReflection {
    filter: gpu::Filter,
    procedural: [Procedural; 2],
    current: usize,
    panorama: Option<(wgpu::Texture, gpu::Cube)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Prepared {
    pub baked: bool,
    pub rebind: bool,
    pub switched: bool,
}

impl SkyReflection {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        Self {
            filter: gpu::Filter::new(device, registry),
            procedural: std::array::from_fn(|_| Procedural {
                cube: gpu::Cube::new(device, "Procedural Sky Reflection"),
                holds: None,
            }),
            current: 0,
            panorama: None,
        }
    }

    pub(crate) fn view(&self, panorama: Option<&GpuTexture>) -> &wgpu::TextureView {
        match (&self.panorama, self.procedural_slot(panorama)) {
            (Some((_, cube)), None) => &cube.view,
            _ => &self.procedural[self.current].cube.view,
        }
    }

    pub(crate) fn procedural_slot(&self, panorama: Option<&GpuTexture>) -> Option<usize> {
        match (&self.panorama, panorama) {
            (Some((held, _)), Some(p)) if *held == p.texture => None,
            _ => Some(self.current),
        }
    }

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
        let held = self.procedural.iter().any(|p| p.holds == Some(*sky));
        match (plan(held, held_matches), panorama) {
            (Bake::Panorama, Some(p)) => {
                let cube = gpu::Cube::new(device, "Panorama Sky Reflection");
                self.filter
                    .bake((device, queue), timer, &cube, (Some(p), sky));
                self.panorama = Some((p.texture.clone(), cube));
                Prepared {
                    baked: true,
                    rebind: true,
                    switched: false,
                }
            }
            (_, None) => {
                let holds = self.procedural.each_ref().map(|p| p.holds);
                let (slot, prepared) = procedural_step(&holds, self.current, sky);
                if prepared.baked {
                    let target = &mut self.procedural[slot];
                    self.filter
                        .bake((device, queue), timer, &target.cube, (None, sky));
                    target.holds = Some(*sky);
                }
                self.current = slot;
                prepared
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

#[cfg(test)]
#[path = "settings_gpu_tests.rs"]
mod settings_gpu_tests;

#[cfg(test)]
#[path = "defaults_gpu_tests.rs"]
mod defaults_gpu_tests;
