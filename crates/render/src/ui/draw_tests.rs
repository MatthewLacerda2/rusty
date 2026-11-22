use glam::{Vec2, Vec3, Vec4};

use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_components::{CanvasComponent, ImageComponent, RectTransformComponent};
use rusty_scene::{Camera, Scene};

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
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let (mut scene, bar) = hud();
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    let out = view.color_target_view().expect("offscreen target");
    let frame = |renderer: &mut crate::Renderer, view: &mut RenderView, s: &Scene| {
        renderer.render(view, s, &cam, &out, false);
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

    let mut scene_view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    let target = scene_view.color_target_view().expect("target");
    renderer.render(&mut scene_view, &scene, &cam, &target, true);
    assert_eq!(scene_view.ui.last_drawn(), 0);
    let mut capture = RenderView::targetless(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    renderer.render(&mut capture, &scene, &cam, &target, false);
    assert_eq!(capture.ui.last_drawn(), 0);
    let output = scene_view.color_target_view().expect("target");
    capture.set_ui_output(Some((output, OFFSCREEN_FORMAT)));
    renderer.render(&mut capture, &scene, &cam, &target, false);
    assert_eq!(capture.ui.last_drawn(), 1);
    renderer.render(&mut capture, &scene, &cam, &target, false);
    assert_eq!(capture.ui.last_drawn(), 0);
}

#[test]
fn gpu_the_scene_view_draws_the_screen_ui_with_its_overlay_on() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let (scene, _) = hud();
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    let target = view.color_target_view().expect("target");
    view.ui.screen_in_editor = true;
    renderer.render(&mut view, &scene, &cam, &target, true);
    assert_eq!(view.ui.last_drawn(), 1, "the Scene tab's UI toggle (#423)");
}

#[test]
fn gpu_a_screen_box_confines_the_screen_ui() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(32, 16) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let (mut scene, bar) = hud();
    let mut rt = scene.world.rect_transform_mut(bar).expect("rect");
    (rt.anchor_min, rt.anchor_max) = (Vec2::ZERO, Vec2::ONE);
    drop(rt);
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 16);
    let target = view.color_target_view().expect("target");
    view.ui.screen_in_editor = true;
    view.ui.screen_box = Some((0, 0, 16, 16));
    renderer.render(&mut view, &scene, &cam, &target, true);
    let texture = view.color_target().expect("texture");
    let img =
        crate::readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, 32, 16);
    let white = |x: usize, y: usize| img[(y * 32 + x) * 4..][..3].iter().all(|&c| c > 240);
    assert!(white(4, 8) && white(12, 8), "the box is covered");
    assert!(!white(20, 8) && !white(28, 8), "right of the box is not");
}
