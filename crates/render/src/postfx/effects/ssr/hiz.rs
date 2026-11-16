//! SSR's min-depth pyramid (#2228, HDRP's Hi-Z for its SSR trace): half the view's
//! size and down to 1×1, each texel the nearest depth along the view axis among
//! the texels of the velocity target it covers (`postfx/ssr_hiz.wgsl`). The march
//! (`lighting/ssr_march.wgsl`'s `clear`) proves a step in front of everything it
//! passes over by a few reads of it, and probes texel by texel only where
//! something nearer lies under the step: so it cannot step over a surface thinner
//! than its step, a 1 px line the ray passes behind, which a probe at the step's
//! end alone found on some of TAA's sub-pixel offsets and missed on others.
//!
//! Not `render::passes::hiz`'s pyramid, which is max-reduced (the farthest depth,
//! for occlusion) and built from last frame's visible set: this one is min-reduced
//! from this frame's full depth. `R16Float`, as the velocity target's depth is: at
//! 1080p it holds 1.4 MB, and building it costs one small fullscreen pass per level,
//! untimed like the colour pyramid. It is the view's, held with SSR's other targets
//! ([`super::pass::SsrTargets`]).

use super::pyramid::levels;
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::texture_entry;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;

/// The pyramid's format: the velocity target's depth lane is `f16` too.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The two min-reduce pipelines (the first level from the velocity target, the
/// rest from the level above), shared by every view.
pub(crate) struct HizPass {
    layout: wgpu::BindGroupLayout,
    first: LazyPipeline,
    rest: LazyPipeline,
}

impl HizPass {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr_hiz.wgsl", "SSR Hi-Z Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Hi-Z Layout"),
            entries: &[texture_entry(0, wgpu::TextureViewDimension::D2)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Hi-Z Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |fs: &'static str| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let target = wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                };
                Fullscreen {
                    label: "SSR Hi-Z Pipeline",
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs,
                }
                .colour(device, &[Some(target)])
            })
        };
        Self {
            first: pipeline("fs_first"),
            rest: pipeline("fs_min"),
            layout,
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.first, &self.rest]
    }
}

/// One view's pyramid: the texture, a view of every level (what the march reads),
/// and each level's target view, with the group reading the level above it (none
/// for the first: [`Hiz::build`] reads the velocity target it is handed).
pub(crate) struct Hiz {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    first: wgpu::TextureView,
    steps: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

impl Hiz {
    /// The pyramid of a view of `size`: its first level half of it.
    pub(crate) fn new(device: &wgpu::Device, pass: &HizPass, (w, h): (u32, u32)) -> Self {
        use wgpu::TextureUsages as U;
        let (width, height) = ((w / 2).max(1), (h / 2).max(1));
        let count = levels((width, height));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR Hi-Z"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            view_formats: &[],
        });
        let level = |mip| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let steps = (1..count)
            .map(|mip| {
                let group = group(device, pass, &level(mip - 1));
                (level(mip), group)
            })
            .collect();
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            first: level(0),
            texture,
            steps,
        }
    }

    /// How many levels it holds: what the march is told (`SsrParams::march.z`).
    pub(crate) fn levels(&self) -> u32 {
        self.texture.mip_level_count()
    }

    /// Record every level: the first from `velocity`, then each from the one above.
    pub(crate) fn build(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pass: &HizPass,
        velocity: &wgpu::TextureView,
    ) {
        let first = group(device, pass, velocity);
        draw(encoder, &self.first, pass.first.get(), &first);
        for (target, group) in &self.steps {
            draw(encoder, target, pass.rest.get(), group);
        }
    }
}

fn group(device: &wgpu::Device, pass: &HizPass, src: &wgpu::TextureView) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SSR Hi-Z Bind Group"),
        layout: &pass.layout,
        entries: &[entry(0, wgpu::BindingResource::TextureView(src))],
    })
}

fn draw(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SSR Hi-Z Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    rp.set_pipeline(pipeline);
    rp.set_bind_group(0, group, &[]);
    rp.draw(0..3, 0..1);
}
