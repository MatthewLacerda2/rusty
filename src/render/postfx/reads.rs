//! Which image each stage of the chain reads (#1847): one place that assembles the
//! scene view from this frame's targets — depth of field, the TAA resolve, its
//! contrast-adaptive sharpen (#1847) and motion blur — in URP's order, plus bloom's
//! aux view and the exposure meter's borrow-split scene.

use super::{effects, PfxTarget, PostFx};

impl PostFx {
    /// Bloom's result (io binding 3): the top of its chain while the view blooms,
    /// else the composite textures' black texel, so a view that never blooms holds
    /// no chain (#1700).
    pub(crate) fn bloom_view(&self) -> &wgpu::TextureView {
        let chain = self.targets.get::<effects::bloom::BloomTargets>();
        chain.map_or(self.effect_textures.black(), |t| t.view())
    }

    /// [`scene_of`] over this frame's `blur` and `sharpen` targets, the TAA output
    /// and depth of field: the one place the chain's reads are assembled.
    fn scene_view<'a>(
        &'a self,
        blur: Option<&'a effects::motion_blur::BlurTargets>,
        sharpen: Option<&'a effects::taa::SharpenTargets>,
    ) -> &'a wgpu::TextureView {
        scene_of(
            blur,
            self.taa(),
            sharpen,
            self.targets.get(),
            &self.scene_hdr,
        )
    }

    /// The scene the composite and bloom read: motion-blurred (#1436), else the
    /// sharpened/resolved scene.
    pub(crate) fn scene_color(&self) -> &wgpu::TextureView {
        self.scene_view(self.targets.get(), self.targets.get())
    }

    /// The scene motion blur reads: the contrast-adaptive sharpen's output when it
    /// ran (#1847), else the TAA resolve, else [`PostFx::focused_color`].
    pub(crate) fn sharpened_color(&self) -> &wgpu::TextureView {
        self.scene_view(None, self.targets.get())
    }

    /// The scene the sharpen reads: this frame's TAA resolve when it ran (#1702),
    /// else [`PostFx::focused_color`], before any sharpen.
    pub(crate) fn resolved_color(&self) -> &wgpu::TextureView {
        self.scene_view(None, None)
    }

    /// The scene TAA reads: this frame's depth of field when it ran (#1445), else
    /// the HDR scene itself. Depth of field runs first, Unity's order.
    pub(crate) fn focused_color(&self) -> &wgpu::TextureView {
        scene_of(None, None, None, self.targets.get(), &self.scene_hdr)
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
        let scene = scene_of(
            self.targets.get(),
            taa,
            self.targets.get(),
            self.targets.get(),
            &self.scene_hdr,
        );
        (&mut self.exposure, (scene, size))
    }
}

/// The motion-blurred image when `blur` ran, else the sharpened one when `sharpen`
/// ran, else TAA's resolve when `taa` ran, else the focused one when `dof` did.
fn scene_of<'a>(
    blur: Option<&'a effects::motion_blur::BlurTargets>,
    taa: Option<&'a PfxTarget>,
    sharpen: Option<&'a effects::taa::SharpenTargets>,
    dof: Option<&'a effects::depth_of_field::DofTargets>,
    scene_hdr: &'a PfxTarget,
) -> &'a wgpu::TextureView {
    let focused = dof.map_or(&scene_hdr.view, |d| &d.focused.view);
    let resolved = taa.map_or(focused, |t| &t.view);
    let sharpened = sharpen.map_or(resolved, |s| s.sharpened_view());
    blur.map_or(sharpened, |b| &b.blurred.view)
}
