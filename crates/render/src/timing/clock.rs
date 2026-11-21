use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use crate::Renderer;

const SHADER: &str = "
@group(0) @binding(0) var<storage, read_write> sink: array<f32, 64>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>,
        @builtin(local_invocation_index) lane: u32) {
    var x = f32(id.x);
    for (var i = 0u; i < 1024u; i++) {
        x = x * 0.9999 + 1.0;
    }
    sink[lane] = x;
}
";

/// Workgroups per probe: enough to fill every core of a laptop GPU.
const WORKGROUPS: u32 = 256;

pub struct ClockProbe {
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period_ns: f64,
}

impl ClockProbe {
    pub fn new(renderer: &Renderer) -> Option<Self> {
        let (device, queue) = (&renderer.device, &renderer.queue);
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let pipeline = pipeline(device);
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let sink = buffer(
            "Bench clock probe sink",
            64 * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let bind_group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bench clock probe"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sink.as_entire_binding(),
            }],
        });
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Bench clock probe"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        use wgpu::BufferUsages as U;
        Some(Self {
            pipeline,
            bind_group,
            set,
            resolve: buffer("Bench probe resolve", 16, U::QUERY_RESOLVE | U::COPY_SRC),
            readback: buffer("Bench probe readback", 16, U::MAP_READ | U::COPY_DST),
            period_ns: f64::from(queue.get_timestamp_period()),
        })
    }

    pub fn time(&self, renderer: &Renderer) -> Option<f64> {
        let (device, queue) = (&renderer.device, &renderer.queue);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Bench clock probe"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &self.set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(crate::gpu::ONLY_GROUP, &self.bind_group, &[]);
            pass.dispatch_workgroups(WORKGROUPS, 1, 1);
        }
        encoder.resolve_query_set(&self.set, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, Some(16));
        queue.counted_submit(Some(encoder.finish()));
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let ticks: Vec<u64> = slice
            .get_mapped_range()
            .ok()?
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes(b.try_into().expect("8-byte chunk")))
            .collect();
        self.readback.unmap();
        let ms = ticks[1].saturating_sub(ticks[0]) as f64 * self.period_ns / 1e6;
        (ms > 0.0).then_some(ms)
    }
}

fn pipeline(device: &wgpu::Device) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Bench clock probe"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Bench clock probe"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
