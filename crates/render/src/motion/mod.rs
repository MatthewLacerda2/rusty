mod history;

pub(crate) use history::{track, HistoryKey, MotionHistory};

use crate::postfx::effects::{depth_of_field::dof_enabled, motion_blur::motion_blur_enabled, taa};
use crate::postfx::params::PostSource;
use crate::postfx::PfxTarget;
use crate::PixelSize;
use rusty_core::settings::GraphicsSettings;

/// rg NDC motion, b linear view depth (world units; not distance, so depth of field focuses on
/// a plane), a the drawing camera's stack index (0 base).
pub(crate) const VELOCITY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// A half float's largest value: the sky's depth. `SKY_DISTANCE` in `sky/motion.wgsl` mirrors it.
pub(crate) const SKY_DISTANCE: f64 = 65504.0;

pub(crate) const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: SKY_DISTANCE,
    a: 0.0,
};

pub(crate) fn wanted(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    full && !scene_view && motion_blur_enabled(post, graphics)
}

pub(crate) fn target_wanted(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    let dof = full && !scene_view && dof_enabled(post, graphics);
    let taa = taa::wanted(post, graphics, full, scene_view);
    dof || taa || wanted(post, graphics, full, scene_view)
}

#[derive(Default)]
pub(crate) struct MotionVectors {
    target: Option<(PfxTarget, PixelSize)>,
    pub(crate) history: MotionHistory,
    recording: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Velocity {
    Off,
    Depths,
    Motion,
}

impl MotionVectors {
    pub(crate) fn begin(
        &mut self,
        device: &wgpu::Device,
        size: PixelSize,
        key: HistoryKey,
        velocity: Velocity,
        cameras: &[(Option<u32>, glam::Mat4)],
    ) -> Vec<glam::Mat4> {
        self.recording = velocity == Velocity::Motion;
        if velocity == Velocity::Off {
            self.target = None;
        } else if self.target.as_ref().is_none_or(|(_, s)| *s != size) {
            self.target = Some((create_target(device, size), size));
        }
        self.history.begin(key, cameras)
    }

    pub(crate) fn target_view(&self) -> Option<&wgpu::TextureView> {
        self.active_target().map(|t| &t.view)
    }

    pub(crate) fn active_target(&self) -> Option<&PfxTarget> {
        self.target.as_ref().map(|(t, _)| t)
    }

    pub(crate) fn active(&self) -> bool {
        self.recording
    }

    pub(crate) fn reset(&mut self) {
        self.history.clear();
    }
}

fn create_target(device: &wgpu::Device, size: PixelSize) -> PfxTarget {
    use wgpu::TextureUsages as U;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Velocity Target"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: VELOCITY_FORMAT,
        usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    PfxTarget { texture, view }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cut_tests;

#[cfg(test)]
pub(crate) mod gpu_fixture;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod stack_gpu_tests;

#[cfg(test)]
mod vectors_gpu_tests;
