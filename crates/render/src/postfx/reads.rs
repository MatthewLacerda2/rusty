//! Which image each stage of the chain reads: one place that assembles the HDR
//! scene view from this frame's targets — depth of field, the TAA resolve and
//! motion blur — in URP's order, plus bloom's aux view and the exposure meter's
//! borrow-split scene. TAA's sharpen (#1847) runs on the LDR image after them.

use super::{effects, PfxTarget, PostFx};

impl PostFx {
    /// Bloom's result (io binding 3): the top of its chain while the view blooms,
    /// else the composite textures' black texel, so a view that never blooms holds
    /// no chain (#1700).
    pub(crate) fn bloom_view(&self) -> &wgpu::TextureView {
        let chain = self.targets.get::<effects::bloom::BloomTargets>();
        chain.map_or(self.effect_textures.black(), |t| t.view())
    }

    /// The scene the composite and bloom read: motion-blurred (#1436), else
    /// [`PostFx::resolved_color`].
    pub(crate) fn scene_color(&self) -> &wgpu::TextureView {
        scene_of(
            self.targets.get(),
            self.taa(),
            self.targets.get(),
            &self.scene_hdr,
        )
    }

    /// The scene motion blur reads: this frame's TAA resolve when it ran (#1702),
    /// else [`PostFx::focused_color`]. URP's order: depth of field, TAA, motion blur.
    pub(crate) fn resolved_color(&self) -> &wgpu::TextureView {
        scene_of(None, self.taa(), self.targets.get(), &self.scene_hdr)
    }

    /// The scene TAA reads: this frame's depth of field when it ran (#1445), else
    /// the HDR scene itself. Depth of field runs first, Unity's order.
    pub(crate) fn focused_color(&self) -> &wgpu::TextureView {
        scene_of(None, None, self.targets.get(), &self.scene_hdr)
    }

    /// TAA's output this frame, when its resolve ran: at the output's size when it
    /// upscaled the view (TAAU), which is then the size of all that reads it.
    fn taa(&self) -> Option<&PfxTarget> {
        let history = self.targets.get::<effects::taa::TaaHistory>();
        history.and_then(|h| h.output())
    }

    /// The exposure meter and [`PostFx::scene_color`] (with its size) at once, so the
    /// meter reads the image after depth of field, TAA and motion blur (#1444)
    /// without a borrow clash.
    pub(crate) fn exposure_and_scene(
        &mut self,
    ) -> (
        &mut effects::exposure::ExposureMeter,
        (&wgpu::TextureView, [u32; 2]),
    ) {
        // The chain's size: the output's when TAA upscaled the view (#1702).
        let size = [self.chain_size.0, self.chain_size.1];
        let taa = (self.targets.get::<effects::taa::TaaHistory>()).and_then(|h| h.output());
        let scene = scene_of(self.targets.get(), taa, self.targets.get(), &self.scene_hdr);
        (&mut self.exposure, (scene, size))
    }
}

/// The motion-blurred image when `blur` ran, else TAA's resolve when `taa` ran,
/// else the focused one when `dof` did.
fn scene_of<'a>(
    blur: Option<&'a effects::motion_blur::BlurTargets>,
    taa: Option<&'a PfxTarget>,
    dof: Option<&'a effects::depth_of_field::DofTargets>,
    scene_hdr: &'a PfxTarget,
) -> &'a wgpu::TextureView {
    let focused = dof.map_or(&scene_hdr.view, |d| &d.focused.view);
    let resolved = taa.map_or(focused, |t| &t.view);
    blur.map_or(resolved, |b| &b.blurred.view)
}
