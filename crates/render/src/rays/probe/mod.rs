//! Traces through a scene's tree on the GPU: given rays (the tests) or one per pixel
//! from a camera, timed (`camera`, `make ray-scene`).

mod camera;
mod timer;

pub use camera::{measure, Clock, Measure};

use std::time::Instant;

#[cfg(test)]
use glam::Vec3;
#[cfg(test)]
use wgpu::util::DeviceExt;

#[cfg(test)]
use super::walk::Hit;
use super::GROUP;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use timer::Timer;

const IO: u32 = 0;
use crate::Renderer;
#[cfg(test)]
use rusty_scene::Scene;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ProbeHit {
    t: f32,
    slot: u32,
    tri: u32,
    any: u32,
    bary: [f32; 2],
    pad: [f32; 2],
}

fn entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage(read_only: bool) -> wgpu::BufferBindingType {
    wgpu::BufferBindingType::Storage { read_only }
}

struct Probe {
    io: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

fn probe(r: &Renderer, entry_point: &str, io: &[wgpu::BindGroupLayoutEntry]) -> Probe {
    let device = &r.device;
    let mut shaders =
        crate::gpu::shaders::ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
    let module = shaders.load(device, "rt/probe.wgsl", "Ray Probe");
    let io = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Ray Probe"),
        entries: io,
    });
    let rays = r.retained.ray_layout(device);
    let mut groups = vec![None; GROUP as usize + 1];
    (groups[IO as usize], groups[GROUP as usize]) = (Some(&io), Some(rays));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Ray Probe"),
        bind_group_layouts: &groups,
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Ray Probe"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    });
    Probe { io, pipeline }
}

fn group(
    r: &Renderer,
    layout: &wgpu::BindGroupLayout,
    buffers: &[(u32, &wgpu::Buffer)],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry> = buffers
        .iter()
        .map(|&(binding, b)| wgpu::BindGroupEntry {
            binding,
            resource: b.as_entire_binding(),
        })
        .collect();
    r.device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Ray Probe"),
        layout,
        entries: &entries,
    })
}

/// Its milliseconds on `timer` when given, else on the wall clock.
fn dispatch(
    r: &Renderer,
    p: &Probe,
    (io, tree): (&wgpu::BindGroup, &wgpu::BindGroup),
    n: (u32, u32),
    timer: Option<&Timer>,
) -> Option<f64> {
    let mut encoder = r.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Ray Probe"),
            timestamp_writes: timer.map(Timer::writes),
        });
        pass.set_pipeline(&p.pipeline);
        pass.set_bind_group(IO, io, &[]);
        pass.set_bind_group(GROUP, tree, &[]);
        pass.dispatch_workgroups(n.0, n.1, 1);
    }
    if let Some(timer) = timer {
        timer.resolve(&mut encoder);
    }
    let start = Instant::now();
    r.queue.counted_submit(std::iter::once(encoder.finish()));
    r.device.poll(wgpu::PollType::wait_indefinitely()).ok();
    let wall = start.elapsed().as_secs_f64() * 1000.0;
    match timer {
        Some(timer) => timer.read(&r.device),
        None => Some(wall),
    }
}

pub(super) fn read(r: &Renderer, buffer: &wgpu::Buffer) -> Vec<u8> {
    let staging = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Ray Probe Read"),
        size: buffer.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
    r.queue.counted_submit(std::iter::once(encoder.finish()));
    staging.slice(..).map_async(wgpu::MapMode::Read, |_| ());
    r.device.poll(wgpu::PollType::wait_indefinitely()).ok();
    let mapped = staging.slice(..).get_mapped_range();
    mapped.map(|m| m.to_vec()).unwrap_or_default()
}

fn rw(r: &Renderer, label: &str, size: u64) -> wgpu::Buffer {
    r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(4),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// Each ray's closest hit, and whether `trace_any` saw something, from the GPU.
#[cfg(test)]
pub(crate) fn trace(
    r: &mut Renderer,
    scene: &Scene,
    rays: &[(Vec3, Vec3, f32)],
) -> Vec<(Option<Hit>, bool)> {
    r.ray_tree(scene).expect("the scene was extracted");
    let p = probe(
        r,
        "cs_rays",
        &[entry(0, storage(true)), entry(1, storage(false))],
    );
    let words: Vec<[f32; 8]> = rays
        .iter()
        .map(|&(o, d, t)| [o.x, o.y, o.z, t, d.x, d.y, d.z, 0.0])
        .collect();
    let input = r
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ray Probe Rays"),
            contents: bytemuck::cast_slice(&words),
            usage: wgpu::BufferUsages::STORAGE,
        });
    let out = rw(
        r,
        "Ray Probe Hits",
        (rays.len() * size_of::<ProbeHit>()) as u64,
    );
    let io = group(r, &p.io, &[(0, &input), (1, &out)]);
    let tree = r
        .retained
        .scene(scene.id())
        .and_then(|s| s.rays.bind_group().cloned());
    let tree = tree.expect("uploaded above");
    let n = ((rays.len() as u32).div_ceil(64), 1);
    dispatch(r, &p, (&io, &tree), n, None);
    let bytes = read(r, &out);
    let hits: &[ProbeHit] = bytemuck::cast_slice(&bytes);
    hits.iter()
        .take(rays.len())
        .map(|h| {
            let hit = (h.tri != super::top::NONE).then_some(Hit {
                t: h.t,
                slot: h.slot,
                tri: h.tri,
                u: h.bary[0],
                v: h.bary[1],
            });
            (hit, h.any != 0)
        })
        .collect()
}
