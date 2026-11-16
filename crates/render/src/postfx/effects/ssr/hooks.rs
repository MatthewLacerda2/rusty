//! Test-only: what SSR's GPU tests read of a view's SSR state.

use super::pass::SsrTargets;
use crate::postfx::PostFx;

impl PostFx {
    /// Whether the view holds SSR's state (it ran this frame).
    pub(crate) fn holds_ssr(&self) -> bool {
        self.targets.get::<SsrTargets>().is_some()
    }

    /// The trace's hits this frame (`postfx/ssr.wgsl`'s `Traced::hit`: colour times
    /// weight, and the weight), if SSR ran: its first level holds them.
    pub(crate) fn ssr_hits(&self) -> Option<&wgpu::Texture> {
        Some(&self.targets.get::<SsrTargets>()?.hits.texture)
    }

    /// The size and level count of the pyramid SSR read this frame, if it ran.
    pub(crate) fn ssr_pyramid(&self) -> Option<((u32, u32), u32)> {
        let t = self.targets.get::<SsrTargets>()?;
        Some((t.size, t.pyramid.levels()))
    }
}
