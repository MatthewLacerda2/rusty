//! The flare's composite texture on lavapipe (#1473): bound to the flare's target
//! while the view flares, to the registry's black texel once it stops; the dummy's,
//! never provided, always to the black texel.

use super::gpu_tests::{eye, renderer, scene, H, W};
use crate::core::settings::quality::QualityPreset;
use crate::render::postfx::effects::textures::{bound_views, composite_textures};
use crate::render::{RenderView, OFFSCREEN_FORMAT};

/// Where `effect`'s one texture sits in group 1.
fn binding(effect: &str) -> usize {
    (composite_textures().iter())
        .position(|t| t.effect.name == effect)
        .unwrap_or_else(|| panic!("`{effect}` declares a composite texture"))
}

#[test]
fn gpu_the_flare_binds_its_target_while_it_flares_and_black_after() {
    let Some(mut r) = renderer(QualityPreset::High) else {
        return;
    };
    let black = r
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Fallback"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let (flare, dummy) = (binding("lens_flare"), binding("dummy"));
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = view.color_target_view().unwrap();

    r.render(&mut view, &scene(40.0, true), &eye(), &out, false);
    let views = bound_views(&view.post_fx, &black);
    let target = &view
        .post_fx
        .targets
        .get::<super::FlareTargets>()
        .unwrap()
        .flare;
    assert!(
        std::ptr::eq(views[flare], &target.view),
        "the flare's target"
    );
    assert!(std::ptr::eq(views[dummy], &black), "the dummy has none");
    assert_eq!(views.len(), composite_textures().len());

    r.render(&mut view, &scene(40.0, false), &eye(), &out, false);
    let views = bound_views(&view.post_fx, &black);
    assert!(std::ptr::eq(views[flare], &black), "no flare: black");
}
