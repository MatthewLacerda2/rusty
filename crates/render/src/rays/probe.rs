//! Traces through a scene's tree on the GPU: given rays (the tests) or one per pixel
//! from a camera, timed (`make ray-scene`).

use std::time::Instant;

#[cfg(test)]
use glam::Vec3;
use wgpu::util::DeviceExt;

#[cfg(test)]
use super::walk::Hit;
use super::GROUP;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};

const IO: u32 = 0;
use crate::Renderer;
use rusty_scene::{Camera, Scene};

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

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Eye {
    inv_view_proj: [[f32; 4]; 4],
    origin: [f32; 4],
    size: [u32; 4],
}

/// What `make ray-scene` prints for one scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub triangles: u64,
    pub instances: u32,
    pub meshes: u32,
    pub gpu_bytes: u64,
    pub build_ms: f64,
    pub trace_ms: f64,
    pub hit_pixels: u64,
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
    first: u32,
    buffers: &[&wgpu::Buffer],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry> = buffers
        .iter()
        .enumerate()
        .map(|(i, b)| wgpu::BindGroupEntry {
            binding: first + i as u32,
            resource: b.as_entire_binding(),
        })
        .collect();
    r.device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Ray Probe"),
        layout,
        entries: &entries,
    })
}

fn dispatch(r: &Renderer, p: &Probe, io: &wgpu::BindGroup, tree: &wgpu::BindGroup, n: (u32, u32)) {
    let mut encoder = r.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.counted_compute_pass(&Default::default());
        pass.set_pipeline(&p.pipeline);
        pass.set_bind_group(IO, io, &[]);
        pass.set_bind_group(GROUP, tree, &[]);
        pass.dispatch_workgroups(n.0, n.1, 1);
    }
    r.queue.counted_submit(std::iter::once(encoder.finish()));
    r.device.poll(wgpu::PollType::wait_indefinitely()).ok();
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
    let io = group(r, &p.io, 0, &[&input, &out]);
    let tree = r
        .retained
        .scene(scene.id())
        .and_then(|s| s.rays.bind_group().cloned());
    dispatch(
        r,
        &p,
        &io,
        &tree.expect("uploaded above"),
        ((rays.len() as u32).div_ceil(64), 1),
    );
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

fn eye(r: &Renderer, camera: &Camera, size: (u32, u32)) -> wgpu::Buffer {
    let view_proj = camera.build_view_projection(size.0 as f32 / size.1.max(1) as f32);
    let eye = Eye {
        inv_view_proj: view_proj.inverse().to_cols_array_2d(),
        origin: camera.position.extend(1.0).to_array(),
        size: [size.0, size.1, 0, 0],
    };
    r.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ray Probe Eye"),
            contents: bytemuck::bytes_of(&eye),
            usage: wgpu::BufferUsages::UNIFORM,
        })
}

/// Builds `scene`'s tree from scratch and traces one ray per pixel of `size` from
/// `camera`, each timed on the wall clock; `None` before the scene's first extract.
pub fn measure(
    r: &mut Renderer,
    scene: &Scene,
    camera: &Camera,
    size: (u32, u32),
) -> Option<Measure> {
    r.retained.scene_mut(scene.id())?.rays = super::RayTree::default();
    let start = Instant::now();
    let tree = r.ray_tree(scene)?;
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let packed = tree.packed();
    let (gpu_bytes, group_tree) = (tree.gpu_bytes(), tree.bind_group()?.clone());
    let p = probe(
        r,
        "cs_camera",
        &[
            entry(2, wgpu::BufferBindingType::Uniform),
            entry(3, storage(false)),
        ],
    );
    let uniform = eye(r, camera, size);
    let out = rw(r, "Ray Probe Depths", u64::from(size.0 * size.1) * 4);
    let io = group(r, &p.io, 2, &[&uniform, &out]);
    let groups = (size.0.div_ceil(8), size.1.div_ceil(8));
    dispatch(r, &p, &io, &group_tree, groups);
    let mut times: Vec<f64> = (0..5)
        .map(|_| {
            let start = Instant::now();
            dispatch(r, &p, &io, &group_tree, groups);
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    let depths = read(r, &out);
    let depths: &[f32] = bytemuck::cast_slice(&depths);
    Some(Measure {
        triangles: packed.tris.len() as u64,
        instances: packed.instances.len() as u32,
        meshes: mesh_count(r, scene),
        gpu_bytes,
        build_ms,
        trace_ms: times[times.len() / 2],
        hit_pixels: depths.iter().filter(|&&t| t >= 0.0).count() as u64,
    })
}

fn mesh_count(r: &Renderer, scene: &Scene) -> u32 {
    r.retained
        .scene(scene.id())
        .map_or(0, |s| s.rays.mesh_count())
}
