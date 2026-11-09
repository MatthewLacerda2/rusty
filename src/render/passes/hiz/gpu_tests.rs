//! GPU occlusion culling on lavapipe (#1963): a wall leaves the box behind it
//! undrawn, with no bake and with the camera outside every baked cell; and where
//! nothing is hidden, a frame is the same image with it on and off.

use std::sync::Arc;

use glam::Vec3;

use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::lighting::occlusion::bake_occlusion;
use crate::scene::lighting::occlusion::test_level::block;
use crate::scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 64;

/// A wall across the view at z = 0 and a box 5 m behind it: the scene and the box.
pub(super) fn walled() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    block(
        &mut scene,
        "Wall",
        Vec3::new(-6.0, -2.0, -0.2),
        Vec3::new(6.0, 5.0, 0.2),
    );
    let lo = Vec3::new(-0.5, 0.0, -5.5);
    let boxed = block(&mut scene, "Box", lo, lo + Vec3::ONE);
    (scene, boxed)
}

/// Eight metres in front of the wall, looking at it down -Z.
pub(super) fn facing_the_wall() -> Camera {
    Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0)
}

/// One frame of `scene` through `cam` into `view`: its pixels and counters.
pub(super) fn shot(
    r: &mut Renderer,
    view: &mut RenderView,
    scene: &Scene,
    cam: &Camera,
) -> (Vec<u8>, RenderCounters) {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, texture, W, H);
    (px, r.frame_counters)
}

pub(super) fn offscreen(r: &Renderer) -> RenderView {
    RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H)
}

/// The instances the base camera's last frame drew through the cull, per batch.
pub(super) fn drawn(r: &Renderer, view: &RenderView) -> Vec<u32> {
    let args = view.hiz.cameras[0].final_args();
    let staging = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: args.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(args, 0, &staging, 0, args.size());
    r.queue.submit(std::iter::once(encoder.finish()));
    staging.slice(..).map_async(wgpu::MapMode::Read, |_| ());
    r.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let view = staging.slice(..).get_mapped_range().unwrap();
    let words: &[u32] = bytemuck::cast_slice(&view);
    let batches = view.hiz.cameras[0].batches as usize;
    words.chunks_exact(5).take(batches).map(|a| a[1]).collect()
}

#[test]
fn gpu_a_wall_leaves_the_box_behind_it_undrawn_with_no_bake() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (scene, _) = walled();
    let mut view = offscreen(&r);
    let cam = facing_the_wall();
    let (on, c) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(c.hiz_tested, 2, "the wall and the box: {c:?}");
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 1, "the wall alone");
    // The count comes back without waiting: by a later frame it is in.
    let (_, c) = shot(&mut r, &mut view, &scene, &cam);
    let (_, c2) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(c.hiz_culled.max(c2.hiz_culled), 1, "{c:?}");
    r.quality.gpu_occlusion_culling = false;
    let (off, c) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(c.hiz_tested, 0, "the preset turns it off");
    assert!(on == off, "a hidden box moved pixels");
}

#[test]
fn gpu_a_camera_outside_every_baked_cell_still_culls() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (mut scene, _) = walled();
    let data = bake_occlusion(&scene, 4).0.expect("the wall occludes");
    let cam = Camera::new(Vec3::new(0.0, 1.0, 40.0), -90.0, 0.0);
    assert!(
        data.grid.index_of(cam.position).is_none(),
        "the eye is outside"
    );
    *scene.occlusion = Some(Arc::new(data));
    let mut view = offscreen(&r);
    let (_, c) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(c.occlusion_culled, 0, "the PVS culls nothing out here");
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 1, "the box is culled");
}

#[test]
fn gpu_where_nothing_is_hidden_a_frame_is_the_same_on_and_off() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    for k in 0..12 {
        let at = Vec3::new(
            (k % 4) as f32 * 1.5 - 2.25,
            (k / 4) as f32 * 1.2,
            -(k as f32) * 0.3,
        );
        block(&mut scene, "Crate", at, at + Vec3::splat(0.8));
    }
    let cam = facing_the_wall();
    let mut view = offscreen(&r);
    let (on, c) = shot(&mut r, &mut view, &scene, &cam);
    let all = c.hiz_tested;
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), all, "nothing hidden");
    let (again, _) = shot(&mut r, &mut view, &scene, &cam);
    assert!(
        on == again,
        "a second frame, from last frame's set, differs"
    );
    r.quality.gpu_occlusion_culling = false;
    let (off, _) = shot(&mut r, &mut offscreen(&r), &scene, &cam);
    assert!(on == off, "Hi-Z moved pixels where nothing is hidden");
}
