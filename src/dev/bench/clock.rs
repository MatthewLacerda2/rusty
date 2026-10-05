//! How fast the GPU is clocked right now, measured with a fixed load (#862).
//!
//! The harness steps the sim on the CPU, then renders and waits for the GPU, so
//! the GPU idles part of every frame. Apple silicon's GPU governor reads that idle
//! time and moves the clock (338-1470 MHz on an M4, re-decided many times a
//! second), and a lighter frame idles longer and runs slower. GPU milliseconds
//! then measure the governor as much as the work.
//!
//! [`ClockProbe`] times the same small arithmetic dispatch right before and right
//! after every frame. Its duration follows the clock and nothing else, so scaling
//! the frame's GPU time by [`speed`] — how fast the probe ran against
//! [`REFERENCE_MS`] — divides the clock out: the result is what the frame would
//! have taken at the reference clock.

/// The probe's time at the reference clock: what it takes on an Apple M4 at its
/// top GPU clock (1470 MHz), so on that machine the bench's GPU rows read as ms
/// at full speed. On any other GPU they are that GPU's own consistent unit.
pub const REFERENCE_MS: f64 = 0.13;

/// How fast the GPU ran over a frame against the reference clock (1.0 = the
/// reference, 0.5 = half as fast), from the probe timed before and after it.
/// `None` when neither probe read.
pub fn speed(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    let probe_ms = match (before, after) {
        (Some(a), Some(b)) => (a + b) / 2.0,
        (Some(ms), None) | (None, Some(ms)) => ms,
        (None, None) => return None,
    };
    Some(REFERENCE_MS / probe_ms)
}

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
    /// A probe for `device`; `None` when it has no timestamp queries.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bench clock probe"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bench clock probe"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
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
    pub fn time(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<f64> {
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

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
