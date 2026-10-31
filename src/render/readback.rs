//! src/render/readback.rs — offscreen colour target → CPU bytes → PNG.
//!
//! The one place the engine copies a rendered texture back to the CPU. Its callers
//! — the dev screenshot (`dev/screenshot.rs`), the cubemap capture
//! (`render/ibl/cubemap_capture.rs`), the headless asset preview (`dev/preview.rs`,
//! #353) and the reflection probes' GPU prefilter (`reflection_prefilter::gpu`,
//! #1602) — had each grown (or were about to grow) a copy of the same fiddly
//! alignment dance.
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
    read_texture_bytes(device, queue, texture, (width, height), 4)
}

/// [`read_texture_rgba8`] for any single-plane colour format of `texel_bytes` bytes
/// per texel (8 for the HDR target's `Rgba16Float`): the raw texels, tightly packed.
pub(crate) fn read_texture_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: (u32, u32),
    texel_bytes: u32,
) -> Vec<u8> {
    read_level(device, queue, (texture, 0, 0), size, texel_bytes)
}

/// [`read_texture_rgba8`] of mip level `mip` in array layer `layer` (0 for a plain
/// texture; the decal atlas has many, #1364), `width × height` being that level's
/// region (#1322).
#[cfg(test)]
pub(crate) fn read_mip_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    level: (&wgpu::Texture, u32, u32),
    size: (u32, u32),
) -> Vec<u8> {
    read_level(device, queue, level, size, 4)
}

/// [`read_mip_rgba8`] for any single-plane colour format of `texel_bytes` bytes per
/// texel (8 for the sky reflection cube's `Rgba16Float`, #1553).
#[cfg(test)]
pub(crate) fn read_mip_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    level: (&wgpu::Texture, u32, u32),
    size: (u32, u32),
    texel_bytes: u32,
) -> Vec<u8> {
    read_level(device, queue, level, size, texel_bytes)
}

/// The texels of `size`'s top-left region of `texture`'s level `mip` in array
/// layer `layer`, packed.
fn read_level(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    level: (&wgpu::Texture, u32, u32),
    (width, height): (u32, u32),
    texel_bytes: u32,
) -> Vec<u8> {
    let unpadded_bytes_per_row = width * texel_bytes;
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
    submit_copy(device, queue, level, &buffer, padded_bytes_per_row, size);
    map_and_unpad(
        device,
        &buffer,
        padded_bytes_per_row,
        unpadded_bytes_per_row,
        height,
    )
}

/// Encode and submit the level → padded-buffer copy of `size`'s top-left region.
fn submit_copy(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    (texture, mip_level, layer): (&wgpu::Texture, u32, u32),
    buffer: &wgpu::Buffer,
    padded_bytes_per_row: u32,
    size: wgpu::Extent3d,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Readback Copy Encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer,
            layout: wgpu::TexelCopyBufferLayout {
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
    let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    map(device, buffer, |mapped| {
        for row in mapped.chunks(padded_bytes_per_row as usize) {
            pixels.extend_from_slice(&row[..unpadded_bytes_per_row as usize]);
        }
    });
    pixels
}

/// Map the readback buffer (blocking), hand its bytes to `read`, and unmap it.
fn map(device: &wgpu::Device, buffer: &wgpu::Buffer, read: impl FnOnce(&[u8])) {
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.recv();

    // The buffer was mapped just above, so this only fails on a wgpu bug.
    let mapped = slice
        .get_mapped_range()
        .expect("the readback buffer was just mapped");
    read(&mapped);
    drop(mapped);
    buffer.unmap();
}

/// Every mip of a six-layer RGBA8 `texture` (a cube's faces), level `i` being
/// `sizes[i]` square, in one copy and one map (#1602): `levels[i][layer]`, each
/// tightly packed, row-major, top to bottom.
pub(crate) fn read_cube_mips_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    sizes: &[u32],
) -> Vec<[Vec<u8>; 6]> {
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let pitch = |size: u32| (size * 4).div_ceil(align) * align;
    let offsets: Vec<u64> = sizes
        .iter()
        .scan(0u64, |at, &size| {
            let offset = *at;
            *at += u64::from(pitch(size) * size) * 6;
            Some(offset)
        })
        .collect();
    let total: u64 = sizes.iter().map(|&s| u64::from(pitch(s) * s) * 6).sum();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Cube Readback Buffer"),
        size: total,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Cube Readback Copy"),
    });
    for (mip_level, (&size, &offset)) in (0..).zip(sizes.iter().zip(&offsets)) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(pitch(size)),
                    rows_per_image: Some(size),
                },
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
        );
    }
    queue.submit(std::iter::once(encoder.finish()));
    let mut levels = Vec::with_capacity(sizes.len());
    map(device, &buffer, |bytes| {
        for (&size, &offset) in sizes.iter().zip(&offsets) {
            let (pitch, row, rows) = (pitch(size) as usize, (size * 4) as usize, size as usize);
            levels.push(std::array::from_fn(|layer| {
                let start = offset as usize + layer * pitch * rows;
                bytes[start..start + pitch * rows]
                    .chunks(pitch)
                    .flat_map(|r| &r[..row])
                    .copied()
                    .collect()
            }));
        }
    });
    levels
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
        // lint: allow(asset-write) a screenshot capture, not read by a path-keyed cache
        .save(path)
        .map_err(|e| format!("failed to write PNG {}: {e}", path.display()))
}
