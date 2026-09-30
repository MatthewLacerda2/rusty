//! GPU tests for the UI pass's per-view cache (#418): a canvas is re-uploaded only
//! when its geometry changed, and views that must not show UI never draw it.

use glam::{Vec2, Vec3, Vec4};

use crate::components::{CanvasComponent, ImageComponent, RectTransformComponent};
use crate::render::{RenderView, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

fn hud() -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    let bar = scene.add_entity("Bar".to_string());
    let rt = RectTransformComponent::default();
    scene.world.set_rect_transform(bar, Some(rt));
    scene.world.set_image(bar, Some(ImageComponent::default()));
    scene.set_parent(bar, Some(root)).expect("parent exists");
    (scene, bar)
}

#[test]
fn gpu_a_static_hud_uploads_once_and_a_change_reuploads() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let (mut scene, bar) = hud();
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    let out = view.color_target_view().expect("offscreen target");
    let frame = |renderer: &mut crate::render::Renderer, view: &mut RenderView, s: &Scene| {
        renderer.render(view, s, &cam, &out, false, &[]);
        view.ui.last_uploads()
    };
    assert_eq!(
        frame(&mut renderer, &mut view, &scene),
        1,
        "first frame uploads"
    );
    assert_eq!(
        frame(&mut renderer, &mut view, &scene),
        0,
        "unchanged: no upload"
    );
    scene.world.image_mut(bar).expect("image").color = Vec4::new(1.0, 0.0, 0.0, 1.0);
    assert_eq!(
        frame(&mut renderer, &mut view, &scene),
        1,
        "a graphic change re-uploads"
    );
    let mut rt = scene.world.rect_transform_mut(bar).expect("rect");
    rt.size_delta = Vec2::splat(300.0);
    drop(rt);
    assert_eq!(
        frame(&mut renderer, &mut view, &scene),
        1,
        "a layout change re-uploads"
    );

    // The Scene view (editor mode) never draws the game's UI.
    let mut scene_view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    let target = scene_view.color_target_view().expect("target");
    renderer.render(&mut scene_view, &scene, &cam, &target, true, &[]);
    assert_eq!(scene_view.ui.last_uploads(), 0);
    // A targetless view (the cubemap capture's) has no frame of its own to draw on.
    let mut capture = RenderView::targetless(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    renderer.render(&mut capture, &scene, &cam, &target, false, &[]);
    assert_eq!(capture.ui.last_uploads(), 0);
}
