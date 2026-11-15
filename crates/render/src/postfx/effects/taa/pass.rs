//! What the resolve's pipelines write (`ResolveOut` in `postfx/taa.wgsl`): the
//! history's colour, surface and flicker frames at once, so next frame reads them
//! all from the same resolve (#1846, #2228). `PostFx::fullscreen_into` draws into
//! them.

use super::history::{FLICKER_FORMAT, SURFACE_FORMAT};
use crate::postfx::HDR_FORMAT;

/// What the resolve's pipelines write: the colour frame, its surfaces, its flickers.
pub(super) fn targets() -> [Option<wgpu::ColorTargetState>; 3] {
    let target = |format| wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    };
    [
        Some(target(HDR_FORMAT)),
        Some(target(SURFACE_FORMAT)),
        Some(target(FLICKER_FORMAT)),
    ]
}
