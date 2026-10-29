//! `vertex_waves` on a GPU (#1524): its variant builds the depth pipelines that move
//! the vertex as `vs_main` does, and its analytic normals are the surface's own. The
//! helper runs as written in the catalog, in a compute probe. Skip with no adapter.

use crate::shadergen::blocks::find;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

/// The variant builds: its vertex stages read the amplitude from the material's
/// param uniform and `vs_shadow` reads `camera.time`, so both bindings must be
/// visible to the vertex stage, or wgpu refuses and the water draws flat.
#[test]
fn gpu_vertex_waves_builds_its_depth_pipelines() {
    let name = format!("test_vertex_waves_{}", std::process::id());
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: name.clone(),
        blocks: vec![BlockSel {
            id: "vertex_waves".into(),
            params: Default::default(),
        }],
    };
    bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
    let r = crate::render::test_gpu::headless_or_skip(16, 16);
    let built = r.map(|mut r| {
        let id = r.surface_shaders.pipeline_id(&r.device, Some(&name));
        r.surface_shaders.cut(id).is_some()
    });
    for ext in ["wgsl", "params.json"] {
        let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{name}.{ext}"));
    }
    assert_ne!(
        built,
        Some(false),
        "the variant and its depth pipelines build"
    );
}

/// Per sample: the helper's normal, then the normal of a central difference of its
/// positions across the surface. Row 0 is a level surface with Gerstner crests
/// (`steepness` 0.5); row 1 a tilted one with sine swells.
const PROBE: &str = "
struct SurfaceVertex { position: vec3<f32>, normal: vec3<f32>, tangent: vec3<f32> };
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
fn at(p: vec3<f32>, n: vec3<f32>, steep: f32) -> SurfaceVertex {
    return vtx_vertex_waves(SurfaceVertex(p, n, vec3<f32>(1.0, 0.0, 0.0)), 1.7, 0.2, 2.0, 0.5, vec2<f32>(1.0, 0.3), 3.0, steep);
}
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let tilted = id.y == 1u;
    let n = select(vec3<f32>(0.0, 1.0, 0.0), normalize(vec3<f32>(1.0, 1.0, 0.3)), tilted);
    let steep = select(0.5, 0.0, tilted);
    let u = normalize(cross(n, vec3<f32>(0.0, 0.0, 1.0)));
    let w = cross(u, n);
    let f = f32(id.x);
    let p = u * (f * 0.093 - 3.0) + w * (f * 0.057 - 1.5);
    let e = 0.002;
    let du = at(p + u * e, n, steep).position - at(p - u * e, n, steep).position;
    let dw = at(p + w * e, n, steep).position - at(p - w * e, n, steep).position;
    let i = (id.y * 64u + id.x) * 2u;
    out[i] = vec4<f32>(at(p, n, steep).normal, 0.0);
    out[i + 1u] = vec4<f32>(normalize(cross(dw, du)), 0.0);
}
";

#[test]
fn gpu_vertex_waves_normals_match_a_finite_difference() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let helper = find(PassKind::Surface, "vertex_waves").unwrap().helper;
    let out = run_probe(&r.device, &r.queue, &format!("{helper}\n{PROBE}"));
    let mut tilted_any = false;
    for (k, pair) in out.chunks(2).enumerate() {
        let (analytic, fd) = (glam::Vec4::from(pair[0]), glam::Vec4::from(pair[1]));
        let up = if k < 64 {
            glam::Vec4::Y
        } else {
            glam::vec4(1.0, 1.0, 0.3, 0.0).normalize()
        };
        tilted_any |= analytic.dot(up) < 0.99;
        assert!(analytic.dot(fd) > 0.9995, "sample {k}: {analytic} vs {fd}");
    }
    assert!(tilted_any, "the waves bend the normal");
}

/// Dispatch `source`'s `main` over 64 × 2 samples and read back its 256 `vec4`s.
fn run_probe(device: &wgpu::Device, queue: &wgpu::Queue, source: &str) -> Vec<[f32; 4]> {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let size = 256 * 16;
    let buffer = |usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let storage = buffer(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
    let staging = buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage.as_entire_binding(),
        }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(64, 2, 1);
    }
    encoder.copy_buffer_to_buffer(&storage, 0, &staging, 0, size);
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let mapped = slice.get_mapped_range().expect("mapped");
    bytemuck::cast_slice(&mapped).to_vec()
}
