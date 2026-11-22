mod effect;
pub(crate) mod history;
pub(crate) mod jitter;
pub(crate) mod params;
mod pass;
pub(crate) mod reactive;
pub(crate) mod sharpen;
mod upscale;

use crate::gpu::ops::CountedQueue as _;
pub(crate) use effect::{runs, wanted, EFFECT};
pub(crate) use history::{begin, chain_size, upscales, TaaFrame, TaaHistory};
pub(crate) use sharpen::Sharpen;

use params::TaaParams;
pub(crate) use pass::TaaPasses;

use crate::postfx::PostFx;
use crate::timing::{GpuPass, GpuTimer};

impl PostFx {
    /// `draw_stack` gates the history and the velocity target on the same `taa::wanted`.
    pub(crate) fn run_taa(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        passes: &TaaPasses,
        timer: &GpuTimer,
        (tune, lenses): (params::TaaTune, [[f32; 4]; params::MAX_CAMERAS]),
    ) {
        if let Some(h) = self.targets.get_mut::<TaaHistory>() {
            h.hold_flickers(device, tune.anti_flicker > 0.0);
        }
        let (Some(h), Some(velocity)) =
            (self.targets.get::<TaaHistory>(), self.motion.target_view())
        else {
            return;
        };
        let (read, write) = (h.latest, 1 - h.latest);
        let drawn = self.take_reactive_drawn();
        let frame = (h.valid, h.pixels, drawn);
        let sizes = (h.size, self.full_size); // chain-size: scene the frame TAA reads
        let params = TaaParams::new(sizes, frame, tune, lenses);
        #[cfg(test)]
        let params = params.without(h.rules);
        queue.counted_write_buffer(&h.params, 0, bytemuck::bytes_of(&params));
        let current = self.focused_color();
        let group = passes.group(device, h, read, current, velocity);
        let pipeline = match (h.upscale, h.flickers.is_some()) {
            (false, false) => &passes.resolve,
            (true, false) => &passes.upscale,
            (false, true) => &passes.resolve_held,
            (true, true) => &passes.upscale_held,
        };
        let timed = (pipeline.get(), timer.writes(GpuPass::Taa));
        let (colour, surface) = (&h.frames[write].view, &h.surfaces[write].view);
        match &h.flickers {
            None => PostFx::fullscreen_into(encoder, timed, &[&group], [colour, surface]),
            Some(flickers) => {
                let into = [colour, surface, &flickers[write].view];
                PostFx::fullscreen_into(encoder, timed, &[&group], into);
            }
        }
        if drawn {
            reactive::clear(encoder, &h.reactive, timer.writes(GpuPass::Taa));
        }
        let h = self.targets.get_mut::<TaaHistory>().expect("held above");
        (h.latest, h.valid, h.ran) = (write, true, true);
    }

    pub(crate) fn taa_ran(&self) -> bool {
        self.targets.get::<TaaHistory>().is_some_and(|h| h.ran)
    }

    #[cfg(test)]
    pub(crate) fn taa_upscaled(&self) -> bool {
        let history = self.targets.get::<TaaHistory>();
        history.is_some_and(|h| h.ran && h.upscale)
    }
}

#[cfg(test)]
pub(in crate::postfx::effects) mod gpu;

#[cfg(test)]
pub(crate) fn jitter_of(view: &crate::RenderView) -> glam::Vec2 {
    let history = view.post_fx.targets.get::<TaaHistory>();
    history.map_or(glam::Vec2::ZERO, |h| {
        jitter::ndc(h.pixels, view.scene_size()) // chain-size: scene the jitter's
    })
}

#[cfg(test)]
impl PostFx {
    pub(crate) fn taa_latest(&self) -> Option<&wgpu::Texture> {
        let h = self.targets.get::<TaaHistory>()?;
        Some(&h.frames[h.latest].texture)
    }
}
