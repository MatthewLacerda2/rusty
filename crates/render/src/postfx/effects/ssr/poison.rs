//! Test-only: an `inf` in the image SSR reads, which the forward pass's `finite_hdr` never
//! writes, to hold that a non-finite hit is a miss.

use super::pass::SsrTargets;
use crate::gpu::commands::Commands;
use crate::postfx::PostFx;

impl PostFx {
    pub(crate) fn poison_ssr_source(&mut self) {
        self.targets.get_mut::<SsrTargets>().expect("ran").poisoned = true;
    }
}

pub(super) fn write(
    poisoned: bool,
    (queue, commands): (&wgpu::Queue, &mut Commands),
    scene: &wgpu::Texture,
) {
    if !poisoned {
        return;
    }
    // `write_texture` lands before unsubmitted commands, so the opaque pass would overwrite it.
    commands.flush();
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
