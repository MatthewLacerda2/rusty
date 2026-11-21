use super::{effects, PfxTarget, PostFx};

impl PostFx {
    pub(crate) fn bloom_view(&self) -> &wgpu::TextureView {
        let chain = self.targets.get::<effects::bloom::BloomTargets>();
        chain.map_or(self.effect_textures.black(), |t| t.view())
    }

    pub(crate) fn scene_color(&self) -> &wgpu::TextureView {
        scene_of(
            self.targets.get(),
            self.taa(),
            self.targets.get(),
            &self.scene_hdr,
        )
    }

    pub(crate) fn resolved_color(&self) -> &wgpu::TextureView {
        scene_of(None, self.taa(), self.targets.get(), &self.scene_hdr)
    }

    pub(crate) fn focused_color(&self) -> &wgpu::TextureView {
        scene_of(None, None, self.targets.get(), &self.scene_hdr)
    }

    fn taa(&self) -> Option<&PfxTarget> {
        let history = self.targets.get::<effects::taa::TaaHistory>();
        history.and_then(|h| h.output())
    }

    pub(crate) fn exposure_and_scene(
        &mut self,
    ) -> (
        &mut effects::exposure::ExposureMeter,
        (&wgpu::TextureView, [u32; 2]),
    ) {
        let size = [self.chain_size.0, self.chain_size.1];
        let taa = (self.targets.get::<effects::taa::TaaHistory>()).and_then(|h| h.output());
        let scene = scene_of(self.targets.get(), taa, self.targets.get(), &self.scene_hdr);
        (&mut self.exposure, (scene, size))
    }
}

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
