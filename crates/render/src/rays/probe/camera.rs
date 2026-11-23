//! One ray a pixel from a camera, timed.

use std::time::Instant;

use wgpu::util::DeviceExt;

use super::counts::{per_ray, PerRay};
use super::{dispatch, entry, group, probe, read, rw, storage, ProbeHit};
use crate::gpu::ops::CountedQueue as _;
use crate::timing::Stamps;
use crate::Renderer;
use rusty_scene::{Camera, Scene};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Eye {
    inv_view_proj: [[f32; 4]; 4],
    origin: [f32; 4],
    size: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Timestamps around the pass: traversal alone.
    Gpu,
    /// Around a submit and its wait: the adapter has no timestamps, or its ticks read equal.
    Wall,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub triangles: u64,
    pub instances: u32,
    pub meshes: u32,
    pub gpu_bytes: u64,
    /// From scratch, its upload waited for.
    pub build_ms: f64,
    /// Writing 4 bytes a pixel, the hit distance.
    pub lean_ms: f64,
    /// Writing the whole hit, 32 bytes a pixel.
    pub full_ms: f64,
    pub clock: Clock,
    pub pixels: u64,
    pub hit_pixels: u64,
    pub per_ray: PerRay,
}

impl Measure {
    /// A ray a pixel in `ms`.
    pub fn rays_per_second(&self, ms: f64) -> f64 {
        self.pixels as f64 / (ms / 1000.0)
    }
}

fn inv_view_proj(camera: &Camera, size: (u32, u32)) -> glam::Mat4 {
    let view_proj = camera.build_view_projection(size.0 as f32 / size.1.max(1) as f32);
    view_proj.inverse()
}

fn eye(r: &Renderer, camera: &Camera, size: (u32, u32)) -> wgpu::Buffer {
    let eye = Eye {
        inv_view_proj: inv_view_proj(camera, size).to_cols_array_2d(),
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

/// Builds `scene`'s tree anew and traces each pixel of `size` from `camera`, lean and
/// full, each the median of five, and counts a sample of the rays' work on the CPU
/// twin; `None` before the scene's first extract.
pub fn measure(
    r: &mut Renderer,
    scene: &Scene,
    camera: &Camera,
    size: (u32, u32),
) -> Option<Measure> {
    r.retained.scene_mut(scene.id())?.rays = crate::rays::RayTree::default();
    let start = Instant::now();
    r.ray_tree(scene)?;
    r.queue.counted_submit(std::iter::empty());
    r.device.poll(wgpu::PollType::wait_indefinitely()).ok();
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let tree = &r.retained.scene(scene.id())?.rays;
    let packed = tree.packed();
    let (gpu_bytes, group_tree) = (tree.gpu_bytes(), tree.bind_group()?.clone());
    let timer = Stamps::new(&r.device, &r.queue);
    let mut clock = Clock::Gpu;
    let uniform = eye(r, camera, size);
    let pixels = u64::from(size.0) * u64::from(size.1);
    let groups = (size.0.div_ceil(8), size.1.div_ceil(8));
    let mut time = |entry_point, binding, bytes| {
        let io = [
            entry(2, wgpu::BufferBindingType::Uniform),
            entry(binding, storage(false)),
        ];
        let p = probe(r, entry_point, &io);
        let out = rw(r, "Ray Probe Out", pixels * bytes);
        let io = group(r, &p.io, &[(2, &uniform), (binding, &out)]);
        let run = || dispatch(r, &p, (&io, &group_tree), groups, timer.as_ref());
        run();
        let runs: Vec<(f64, Clock)> = (0..5).map(|_| run()).collect();
        if runs.iter().any(|&(_, c)| c == Clock::Wall) {
            clock = Clock::Wall;
        }
        let mut times: Vec<f64> = runs.iter().map(|&(ms, _)| ms).collect();
        times.sort_by(f64::total_cmp);
        (times.get(times.len() / 2).copied().unwrap_or(f64::NAN), out)
    };
    let (lean_ms, depths) = time("cs_camera", 3, 4);
    let (full_ms, _) = time("cs_camera_full", 4, size_of::<ProbeHit>() as u64);
    let depths = read(r, &depths);
    let depths: &[f32] = bytemuck::cast_slice(&depths);
    Some(Measure {
        triangles: packed.tris.len() as u64,
        instances: packed.instances.len() as u32,
        meshes: mesh_count(r, scene),
        gpu_bytes,
        build_ms,
        lean_ms,
        full_ms,
        clock,
        pixels,
        hit_pixels: depths.iter().filter(|&&t| t >= 0.0).count() as u64,
        per_ray: per_ray(&packed, inv_view_proj(camera, size), camera.position, size),
    })
}

fn mesh_count(r: &Renderer, scene: &Scene) -> u32 {
    r.retained
        .scene(scene.id())
        .map_or(0, |s| s.rays.mesh_count())
}
