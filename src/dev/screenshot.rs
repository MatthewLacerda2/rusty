//! src/dev/screenshot.rs — Offscreen render -> PNG (the agent's eyes)
//!
//! The ONLY place the dev layer touches the renderer. Builds a wgpu device with NO
//! window surface (`Renderer::new_headless`), renders ONE frame into an offscreen
//! colour texture via the same `Renderer::render` the editor uses, copies the
//! texture back to the CPU, and writes a PNG (via the `image` crate, already a
//! dependency). Lets an agent literally SEE a frame and judge lighting / SSR /
//! shadows against the CS1.6 -> FEAR -> Trepang2 visual bar.
//!
//! Runtime caveat: needs a GPU or software adapter (e.g. lavapipe) in the
//! container. When none is available, `capture`/`capture_world` return
//! `Ok(false)` after a clear log — they NEVER panic, and the rest of the dev layer
//! (Step / StepUntil / state) needs no GPU at all.
//!
//! Allowed deps: render (headless path), api.

use std::path::Path;

use crate::app::GameWorld;
use crate::render::{Camera, Renderer, OFFSCREEN_FORMAT};
use crate::scene::Scene;

/// Default screenshot dimensions (16:9), matching the editor window aspect.
pub const DEFAULT_WIDTH: u32 = 1280;
pub const DEFAULT_HEIGHT: u32 = 720;

/// Render `scene` from `camera` into a PNG at `path`, `width` x `height`.
///
/// Returns `Ok(true)` when a frame was captured and written, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully), or `Err` only on an
/// actual I/O / encode failure once a frame has been produced.
pub fn capture(
    scene: &Scene,
    camera: &Camera,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    let mut renderer = match pollster::block_on(Renderer::new_headless(width, height)) {
        Some(r) => r,
        None => {
            log::warn!(
                "[Screenshot] no GPU/software adapter available — skipping capture of {}",
                path.as_ref().display()
            );
            return Ok(false);
        }
    };

    let (width, height) = (renderer.config.width, renderer.config.height);

    // Offscreen colour target the scene pass draws into, then we copy back from.
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Screenshot Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // Reuse the editor's exact render path (editor_mode = false: no gizmos/grid).
    renderer.render(scene, camera, &target_view, false, &[]);

    let pixels = copy_target_to_cpu(&renderer, &target, width, height);
    write_png(path, width, height, &pixels)?;
    Ok(true)
}

/// Convenience: screenshot the harness's live `GameWorld` (its scene + camera).
pub fn capture_world(
    world: &GameWorld,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    capture(world.scene(), world.camera(), path, width, height)
}

/// Copy the rendered texture back to the CPU as tightly-packed RGBA8 bytes.
///
/// `copy_texture_to_buffer` requires each row to be padded to a multiple of
/// `COPY_BYTES_PER_ROW_ALIGNMENT` (256), so we copy into a padded buffer and then
/// strip the padding back out.
fn copy_target_to_cpu(
    renderer: &Renderer,
    target: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let unpadded_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Screenshot Readback Buffer"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = renderer
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Screenshot Copy Encoder"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit(std::iter::once(encoder.finish()));

    // Map the buffer and block until the GPU signals completion.
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    renderer.device.poll(wgpu::Maintain::Wait);
    let _ = rx.recv();

    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in mapped.chunks(padded_bytes_per_row as usize) {
        pixels.extend_from_slice(&row[..unpadded_bytes_per_row as usize]);
    }
    drop(mapped);
    buffer.unmap();
    pixels
}

/// Write packed RGBA8 `pixels` to `path` as a PNG.
fn write_png(path: impl AsRef<Path>, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let buffer = image::RgbaImage::from_raw(width, height, pixels.to_vec())
        .ok_or_else(|| "screenshot pixel buffer size mismatch".to_string())?;
    buffer
        .save(path.as_ref())
        .map_err(|e| format!("failed to write PNG {}: {e}", path.as_ref().display()))
}
