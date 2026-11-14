//! What the resolve's pipelines write (`ResolveOut` in `postfx/taa.wgsl`): the
//! history's colour and surface frames at once, so next frame reads both from the
//! same resolve (#1846). `PostFx::fullscreen_into` draws into them.

use super::history::SURFACE_FORMAT;
use crate::postfx::HDR_FORMAT;

/// What the resolve's pipelines write: the colour frame, then its surfaces.
pub(super) fn targets() -> [Option<wgpu::ColorTargetState>; 2] {
    let target = |format| wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    };
    [Some(target(HDR_FORMAT)), Some(target(SURFACE_FORMAT))]
}
