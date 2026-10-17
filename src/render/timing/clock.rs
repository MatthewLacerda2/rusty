//! [`ClockProbe`]: how fast the GPU is clocked right now, read off a fixed load
//! (#862; moved here from the bench in #989).
//!
//! Apple silicon's GPU governor moves the clock with how busy the GPU is (338-1470
//! MHz on an M4, re-decided many times a second), so GPU milliseconds measure the
//! governor as much as the work. The probe times the same small arithmetic dispatch
//! with a pair of timestamp queries: its duration follows the clock and nothing
//! else. What to do with that reading (the bench's clock scaling) is the caller's;
//! a caller that calibrates against it depends on [`SHADER`] and [`WORKGROUPS`]
//! staying exactly as they are.

use crate::render::Renderer;

/// The fixed load: every invocation runs a dependent multiply-add chain and writes
/// the result, so the compiler cannot drop it. Pure arithmetic, no memory traffic.
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

/// A fixed compute dispatch timed with a pair of timestamp queries.
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
    /// A probe for `renderer`'s device; `None` when it has no timestamp queries.
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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

    /// Run the probe on an otherwise idle GPU and return how long it took, in ms.
    /// Waits for the GPU: an offline caller's tool, like the harness's frames.
    pub fn time(&self, renderer: &Renderer) -> Option<f64> {
        let (device, queue) = (&renderer.device, &renderer.queue);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Bench clock probe"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &self.set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(WORKGROUPS, 1, 1);
        }
        encoder.resolve_query_set(&self.set, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, Some(16));
        queue.submit(Some(encoder.finish()));
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

/// The fixed load's compute pipeline, its bind group layout inferred from [`SHADER`].
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
