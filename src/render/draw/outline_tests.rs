//! The selection outline is a silhouette around where the selection is drawn
//! (#1142): its hull follows the entity's world matrix, so a child of a moved,
//! turned parent is outlined in place, and a mirrored selection draws through the
//! clockwise twin, so its hull's outer shell never covers the object. Skips with
//! no adapter.

use glam::{Quat, Vec3};

use super::overlays::outline_hull;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const RES: u32 = 64;

/// A box parented under a moved, turned pivot, selected, mirrored along X when
/// `mirrored`. Returns the scene and the box's world centre.
fn selected_child(mirrored: bool) -> (Scene, Vec3) {
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
    let centre = scene.world_matrix(child).transform_point3(Vec3::ZERO);
    (scene, centre)
}

/// The editor frame looking down -Z at `centre` from 3 units away, as RGBA8.
fn frame(r: &mut Renderer, scene: &Scene, centre: Vec3) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(centre + Vec3::Z * 3.0, -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, true);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES)
}

/// The outline's orange: red high, green half, blue none.
fn is_outline(p: &[u8]) -> bool {
    p[0] > 180 && (60..200).contains(&p[1]) && p[2] < 80
}

/// (outline pixel count, their centroid, whether the centre pixel is outline).
fn outline_stats(px: &[u8]) -> (usize, [f32; 2], bool) {
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

/// The hull is the selection's world matrix grown about its pivot: it keeps the
/// world placement and mirroring, and only scales.
#[test]
fn the_outline_hull_is_the_world_matrix_scaled_about_its_pivot() {
    let world = glam::Mat4::from_scale_rotation_translation(
        Vec3::new(-1.0, 2.0, 1.0),
        Quat::from_rotation_y(0.5),
        Vec3::new(4.0, 1.0, -2.0),
    );
    let hull = outline_hull(world);
    assert!(hull
        .transform_point3(Vec3::ZERO)
        .abs_diff_eq(Vec3::new(4.0, 1.0, -2.0), 1e-5));
    assert!((hull.determinant() / world.determinant() - 1.05f32.powi(3)).abs() < 1e-4);
}
