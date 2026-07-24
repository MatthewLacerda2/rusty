//! src/render/readback.rs — offscreen colour target → CPU bytes → PNG.
//!
//! The one place the engine copies a rendered texture back to the CPU. Three callers
//! need it — the dev screenshot (`dev/screenshot.rs`), the cubemap capture
//! (`render/ibl/cubemap_capture.rs`), and the headless asset preview
//! (`dev/preview.rs`, #353) — and each had grown (or was about to grow) its own copy
//! of the same fiddly alignment dance.
//!
//! That dance is the whole reason this deserves a home: `copy_texture_to_buffer`
//! requires every row padded to [`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`] (256), so a
//! naive readback silently produces skewed images at any width that isn't a multiple
//! of 64 pixels. Getting it right once, in one function, is worth more than three
//! correct-looking copies.
//!
//! Reads no wall-clock and no RNG: output is a pure function of the rendered texture,
//! so this is clean under the determinism guard (and `render` is exempt regardless).

use std::path::Path;

/// Copy `texture`'s top-left `width × height` region back to the CPU as tightly
/// packed RGBA8 bytes (`width * height * 4`), row-major, top to bottom.
///
/// Blocks until the GPU signals the copy is complete. `texture` must have been
/// created with [`wgpu::TextureUsages::COPY_SRC`] and an 8-bit RGBA format.
pub fn read_texture_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let unpadded_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Readback Buffer"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    submit_copy(device, queue, texture, &buffer, padded_bytes_per_row, size);
    map_and_unpad(
        device,
        &buffer,
        padded_bytes_per_row,
        unpadded_bytes_per_row,
        height,
    )
}

/// Encode and submit the texture → padded-buffer copy of `size`'s top-left region.
fn submit_copy(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    buffer: &wgpu::Buffer,
    padded_bytes_per_row: u32,
    size: wgpu::Extent3d,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Readback Copy Encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit(std::iter::once(encoder.finish()));
}

/// Map the readback buffer (blocking) and strip the per-row padding back out.
fn map_and_unpad(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    padded_bytes_per_row: u32,
    unpadded_bytes_per_row: u32,
    height: u32,
) -> Vec<u8> {
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device.poll(wgpu::Maintain::Wait);
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

/// Write packed RGBA8 `pixels` to `path` as a PNG, creating the parent directory if
/// it doesn't exist yet (an agent naming `out/preview.png` shouldn't have to `mkdir`
/// first, #353).
pub fn write_png(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), String> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let buffer = image::RgbaImage::from_raw(width, height, pixels.to_vec())
        .ok_or_else(|| "readback pixel buffer size mismatch".to_string())?;
    buffer
        .save(path)
        .map_err(|e| format!("failed to write PNG {}: {e}", path.display()))
}
