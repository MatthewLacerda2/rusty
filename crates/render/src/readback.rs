use crate::gpu::ops::CountedQueue as _;
use std::path::Path;

pub fn read_texture_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    read_texture_bytes(device, queue, texture, (width, height), 4)
}

pub(crate) fn read_texture_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: (u32, u32),
    texel_bytes: u32,
) -> Vec<u8> {
    read_level(device, queue, (texture, 0, 0), size, texel_bytes)
}

#[cfg(test)]
pub(crate) fn read_mip_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    level: (&wgpu::Texture, u32, u32),
    size: (u32, u32),
) -> Vec<u8> {
    read_level(device, queue, level, size, 4)
}

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
    queue.counted_submit(std::iter::once(encoder.finish()));
}

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

fn map(device: &wgpu::Device, buffer: &wgpu::Buffer, read: impl FnOnce(&[u8])) {
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.recv();

    let mapped = slice
        .get_mapped_range()
        .expect("the readback buffer was just mapped");
    read(&mapped);
    drop(mapped);
    buffer.unmap();
}

pub(crate) fn read_cube_mips(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    (texture, texel): (&wgpu::Texture, u32),
    sizes: &[u32],
) -> Vec<[Vec<u8>; 6]> {
    let pitch = |size: u32| padded_row(size, texel);
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
    submit_cube_copy(device, queue, (texture, &buffer), (sizes, texel), &offsets);
    let mut levels = Vec::with_capacity(sizes.len());
    map(device, &buffer, |bytes| {
        for (&size, &offset) in sizes.iter().zip(&offsets) {
            let (pitch, row, rows) = (pitch(size) as usize, (size * texel) as usize, size as usize);
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

fn submit_cube_copy(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    (texture, buffer): (&wgpu::Texture, &wgpu::Buffer),
    (sizes, texel): (&[u32], u32),
    offsets: &[u64],
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Cube Readback Copy"),
    });
    for (mip_level, (&size, &offset)) in (0..).zip(sizes.iter().zip(offsets)) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(padded_row(size, texel)),
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
    queue.counted_submit(std::iter::once(encoder.finish()));
}

fn padded_row(size: u32, texel: u32) -> u32 {
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    (size * texel).div_ceil(align) * align
}

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
