//! Test-only: an `inf` in the image SSR reads (#2028's review). The forward pass
//! writes no colour past 60000 (`finite_hdr`, #1699), so no scene can put one
//! there; this writes it straight in, after the opaque pass, to hold the trace's
//! rule that a non-finite hit is a miss.

use super::pass::SsrTargets;
use crate::render::postfx::PostFx;

impl PostFx {
    /// From the next frame on, SSR reads `inf` over the image's upper half.
    pub(crate) fn poison_ssr_source(&mut self) {
        self.targets.get_mut::<SsrTargets>().expect("ran").poisoned = true;
    }
}

/// When `poisoned`, write a half-float `inf` over `scene`'s upper half, queued
/// ahead of the SSR pass's copy of it.
pub(super) fn write(poisoned: bool, queue: &wgpu::Queue, scene: &wgpu::Texture) {
    if !poisoned {
        return;
    }
    let size = scene.size();
    let (w, h) = (size.width, size.height / 2);
    let inf = 0x7C00u16.to_le_bytes();
    let bytes: Vec<u8> = inf
        .iter()
        .copied()
        .cycle()
        .take((w * h * 8) as usize)
        .collect();
    let layout = wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(w * 8),
        rows_per_image: None,
    };
    let block = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    queue.write_texture(scene.as_image_copy(), &bytes, layout, block);
}
