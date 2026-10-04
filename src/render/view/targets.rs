//! src/render/view/targets.rs — a view's owned GPU targets: the depth buffer and the
//! offscreen colour target (with the UI pass's display-space alias, #418).

/// The depth target's format — a depth buffer sampled by the decal + post-FX passes,
/// so it needs `TEXTURE_BINDING` alongside `RENDER_ATTACHMENT`.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Allocate a depth texture (+ its default view) at `width` x `height`. Sampled by the
/// decal and post-FX passes, hence `TEXTURE_BINDING`.
pub(super) fn create_depth(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("View Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Allocate the offscreen colour target: sampled by egui (`TEXTURE_BINDING`) and
/// readable back to the CPU (`COPY_SRC`), so one owned target serves both the editor's
/// `egui::Image` and the dev layer's PNG capture instead of each allocating its own.
///
/// Also returns the UI pass's format (#418): an sRGB target is made viewable as its
/// non-sRGB twin so the UI blends in display space. A device without view-format
/// support (some GL drivers) rejects that; it is caught and answered with a plain
/// target, where the UI blends in linear space — slightly off, never broken.
pub(super) fn create_color_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureFormat) {
    let alias = format.remove_srgb_suffix();
    let (aliased, plain) = ([alias], []);
    let desc = |view_formats| wgpu::TextureDescriptor {
        label: Some("View Colour Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats,
    };
    if alias != format {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let texture = device.create_texture(&desc(&aliased));
        if pollster::block_on(scope.pop()).is_none() {
            return (texture, alias);
        }
    }
    (device.create_texture(&desc(&plain)), format)
}
