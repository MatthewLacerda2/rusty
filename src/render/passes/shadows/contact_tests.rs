//! The sun's shadow stays attached to its caster (#665): a unit box resting on the
//! floor darkens the floor right up to its base, with no lit strip at the contact
//! line, and the bias that does so still leaves an open floor and the box's lit
//! faces free of acne from a grazing to an overhead sun. Skips with no adapter.

use glam::{Quat, Vec2, Vec3};

use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const RES: u32 = 192;

/// A `kind` primitive at `position` scaled by `scale`.
fn place(scene: &mut Scene, kind: Primitive, position: Vec3, scale: Vec3) {
    let id = create_entity(scene, "Shape", Some(kind));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
}

/// No sky or ambient, so a shadowed pixel reads 0; the sun `elevation` degrees up,
/// shining toward +X, and the floor (`floor`: a single-sided `Plane` or a closed
/// slab) with its top at y = 0. A unit box rests on it at the origin when `boxed`.
pub(super) fn yard(elevation: f32, floor: Primitive, boxed: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let e = elevation.to_radians();
    let dir = Vec3::new(e.cos(), -e.sin(), 0.0);
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
    match floor {
        Primitive::Plane => place(&mut scene, floor, Vec3::ZERO, Vec3::ONE),
        _ => place(
            &mut scene,
            floor,
            Vec3::NEG_Y * 0.5,
            Vec3::new(30.0, 1.0, 30.0),
        ),
    }
    if boxed {
        place(&mut scene, Primitive::Box, Vec3::Y * 0.5, Vec3::ONE);
    }
    scene
}

/// A camera at `eye` looking at `target`.
pub(super) fn looking(eye: Vec3, target: Vec3) -> Camera {
    let d = (target - eye).normalize();
    Camera::new(eye, d.z.atan2(d.x).to_degrees(), d.y.asin().to_degrees())
}

/// Summed RGB of the pixel at each of `points`, as `cam` sees `scene`.
pub(super) fn brightness(
    r: &mut Renderer,
    scene: &Scene,
    cam: &Camera,
    points: &[Vec3],
) -> Vec<u32> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    let target = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES);
    let screen = Vec2::splat(RES as f32);
    points
        .iter()
        .map(|&p| {
            let at = cam.world_to_screen(p, screen).position;
            let (x, y) = (at.x as u32, RES - 1 - at.y as u32);
            let i = ((y * RES + x) * 4) as usize;
            px[i..i + 3].iter().map(|&c| u32::from(c)).sum()
        })
        .collect()
}

/// Floor points on the box's shadow side, `d` metres out from its base along +X.
const OUT: [f32; 7] = [0.01, 0.02, 0.03, 0.05, 0.08, 0.12, 0.2];

/// Floor brightness at each of [`OUT`] next to the box, and a lit floor point
/// beside the box, seen from `eye` over the shadow side.
fn contact_profile(r: &mut Renderer, scene: &Scene, eye: Vec3) -> (Vec<u32>, u32) {
    let cam = looking(eye, Vec3::new(0.6, 0.0, 0.0));
    let mut points: Vec<Vec3> = OUT.iter().map(|d| Vec3::new(0.5 + d, 0.0, 0.0)).collect();
    points.push(Vec3::new(0.6, 0.0, 1.2));
    let mut b = brightness(r, scene, &cam, &points);
    let lit = b.pop().unwrap();
    (b, lit)
}

#[test]
fn gpu_a_resting_box_shadows_the_floor_up_to_its_base() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut gaps = Vec::new();
    for floor in [Primitive::Plane, Primitive::Box] {
        for elevation in [30.0, 45.0, 60.0] {
            let scene = yard(elevation, floor, true);
            // Near (cascade 0) and ~30 m out (a later cascade).
            for eye in [Vec3::new(1.6, 0.9, 0.0), Vec3::new(22.0, 18.0, 0.0)] {
                let (shadow, lit) = contact_profile(&mut r, &scene, eye);
                eprintln!("[contact] {floor:?} {elevation}° eye {eye}: {shadow:?} vs lit {lit}");
                assert!(lit > 150, "control: the floor beside the box is lit");
                let out = OUT.iter().zip(&shadow).filter(|(_, b)| **b * 5 >= lit);
                gaps.extend(out.map(|(d, b)| format!("{floor:?} {elevation}° {d} m: {b}/{lit}")));
            }
        }
    }
    // Before #665 the floor read lit 1–3 cm out from the base in cascade 0.
    assert!(gaps.is_empty(), "lit floor at the contact line: {gaps:?}");
}

#[test]
fn gpu_a_single_sided_plane_casts_a_shadow() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(80.0, Primitive::Box, false);
    place(
        &mut scene,
        Primitive::Plane,
        Vec3::Y * 2.0,
        Vec3::splat(0.1),
    );
    let cam = looking(Vec3::new(0.0, 6.0, 4.0), Vec3::ZERO);
    let b = brightness(
        &mut r,
        &scene,
        &cam,
        &[Vec3::new(0.3, 0.0, 0.0), Vec3::new(0.0, 0.0, 2.5)],
    );
    eprintln!("[plane caster] under {} vs open {}", b[0], b[1]);
    assert!(b[1] > 150, "control: the open floor is lit");
    assert!(b[0] * 5 < b[1], "the plane shadows the floor under it");
}

#[path = "acne_tests.rs"]
mod acne;
