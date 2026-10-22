//! The selection outline is a silhouette around where the selection is drawn
//! (#1142): it follows the entity's world matrix, so a child of a moved, turned
//! parent is outlined in place, and a mirrored selection draws through the
//! clockwise twin, so its outer shell never covers the object. Skips with no
//! adapter.

use glam::{Quat, Vec3};

use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const RES: u32 = 64;

/// A box parented under a moved, turned pivot, selected, mirrored along X when
/// `mirrored`. Returns the scene and the box's world centre.
pub(super) fn selected_child(mirrored: bool) -> (Scene, Vec3) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let pivot = create_entity(&mut scene, "Pivot", None);
    let mut t = scene.world.transform_mut(pivot).unwrap();
    (t.position, t.rotation) = (Vec3::new(4.0, 1.0, -2.0), Quat::from_rotation_y(0.5));
    drop(t);
    let child = create_entity(&mut scene, "Child", Some(Primitive::Box));
    scene.set_parent(child, Some(pivot)).unwrap();
    let sign = if mirrored { -1.0 } else { 1.0 };
    let mut t = scene.world.transform_mut(child).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, 0.0, 1.5), Vec3::new(sign, 1.0, 1.0));
    drop(t);
    scene.selected_entity_id = Some(child);
    let centre = scene
        .world
        .local_to_world(child)
        .transform_point3(Vec3::ZERO);
    (scene, centre)
}

/// The editor frame looking down -Z at `centre` from 3 units away, as RGBA8.
pub(super) fn frame(r: &mut Renderer, scene: &Scene, centre: Vec3) -> Vec<u8> {
    frame_from(r, scene, centre, 3.0)
}

/// The editor frame looking down -Z at `centre` from `distance` away, as RGBA8.
pub(super) fn frame_from(r: &mut Renderer, scene: &Scene, centre: Vec3, distance: f32) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(centre + Vec3::Z * distance, -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, true);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES)
}

/// The outline's orange: red high, green half, blue none.
pub(super) fn is_outline(p: &[u8]) -> bool {
    p[0] > 180 && (60..200).contains(&p[1]) && p[2] < 80
}

/// (outline pixel count, their centroid, whether the centre pixel is outline).
pub(super) fn outline_stats(px: &[u8]) -> (usize, [f32; 2], bool) {
    let hits: Vec<[f32; 2]> = px
        .chunks(4)
        .enumerate()
        .filter(|(_, p)| is_outline(p))
        .map(|(i, _)| [(i as u32 % RES) as f32, (i as u32 / RES) as f32])
        .collect();
    let n = hits.len().max(1) as f32;
    let sum = hits
        .iter()
        .fold([0.0; 2], |a, h| [a[0] + h[0], a[1] + h[1]]);
    let mid = ((RES / 2) * RES + RES / 2) as usize * 4;
    (
        hits.len(),
        sum.map(|s| s / n),
        is_outline(&px[mid..mid + 4]),
    )
}

#[test]
fn gpu_a_mirrored_child_is_outlined_in_place_as_a_silhouette() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let half = RES as f32 / 2.0;
    for mirrored in [false, true] {
        let (scene, centre) = selected_child(mirrored);
        let (count, centroid, filled) = outline_stats(&frame(&mut r, &scene, centre));
        eprintln!("[outline, mirrored {mirrored}] {count} px, centroid {centroid:?}");
        assert!(count > 20, "the selection shows an outline");
        assert!(!filled, "the outline is a silhouette, not a fill");
        let off = (centroid[0] - half).abs().max((centroid[1] - half).abs());
        assert!(
            off < RES as f32 / 8.0,
            "the outline rings the box, {off} px off"
        );
    }
}
