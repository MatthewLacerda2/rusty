//! A material's Render Face (#1615) in the forward passes. A closed, refracting
//! transparent Sphere draws its front faces only, so the image no longer depends on
//! which of its triangles the index order draws last (the issue's stripes: turning
//! the ball half a turn about the view axis moved them). A one-sided `Plane` set to
//! `Both` is drawn from behind and lit as its front is, its normal flipped toward
//! the viewer; `Front` leaves it out. Skips with no adapter.

use glam::{Quat, Vec3};

use crate::components::{MaterialAsset, RefractionModel, RenderFace, RenderMode};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, material, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const RES: u32 = 48;

/// The frame `cam` sees, as RGBA8.
pub(super) fn frame(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES)
}

/// The mean of `px`'s colour channels.
pub(super) fn mean(px: &[u8]) -> f32 {
    let rgb = px.chunks(4).flat_map(|p| &p[..3]);
    rgb.map(|&c| f32::from(c)).sum::<f32>() / (px.len() / 4 * 3) as f32
}

/// The mean per-channel difference of two frames.
fn diff(a: &[u8], b: &[u8]) -> f32 {
    let d = a.iter().zip(b).map(|(x, y)| f32::from(x.abs_diff(*y)));
    d.sum::<f32>() / a.len() as f32
}

/// Entity `id`'s own material, to edit.
pub(super) fn asset(scene: &mut Scene, id: u32) -> &mut MaterialAsset {
    let key = material::ensure_material_key(scene, id).unwrap();
    scene.materials.get_mut(&key).unwrap()
}

/// The issue's plasma ball: a refracting, absorbing transparent Sphere drawing
/// `face`, turned `turn` radians about the view axis, before a lit wall.
fn ball(face: RenderFace, turn: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-0.6);
    let wall = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(wall).unwrap();
    (t.position, t.scale) = (Vec3::NEG_Z * 2.0, Vec3::new(8.0, 8.0, 0.2));
    drop(t);
    let id = create_entity(&mut scene, "Ball", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().rotation = Quat::from_rotation_z(turn);
    let m = asset(&mut scene, id);
    (m.render_mode, m.render_face, m.alpha) = (RenderMode::Transparent, face, 0.6);
    (m.refraction_model, m.ior, m.thickness) = (RefractionModel::Planar, 1.5, 0.3);
    (m.transmittance_color, m.roughness) = ([0.3, 0.8, 1.0], 0.1);
    scene
}

#[test]
fn gpu_a_refracting_ball_shows_no_back_faces() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let cam = Camera::new(Vec3::Z * 1.6, -90.0, 0.0);
    let mut turned = |face| {
        let [a, b] = [0.0, std::f32::consts::PI].map(|turn| frame(&mut r, &ball(face, turn), &cam));
        diff(&a, &b)
    };
    let (front, both) = (turned(RenderFace::Front), turned(RenderFace::Both));
    eprintln!("[render face] a half turn changes the ball by {front:.2} (Front), {both:.2} (Both)");
    assert!(
        both > 2.0 * front + 1.0,
        "control: with back faces the order shows"
    );
    assert!(
        front < 1.0,
        "front faces alone draw the same ball either way round"
    );
}

/// A `Plane` (its front faces +Y) under or over a sun shining `up` or down, seen
/// from below or, with `above`, from above; drawing `face`.
pub(super) fn card(face: RenderFace, above: bool, up: bool) -> (Scene, Camera) {
    let mut scene = Scene::new();
    (scene.skybox_path, scene.ambient_intensity) = (String::new(), 0.0);
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let tilt = if up { 1.2 } else { -1.2 };
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(tilt);
    let id = create_entity(&mut scene, "Card", Some(Primitive::Plane));
    let m = asset(&mut scene, id);
    (m.render_face, m.roughness) = (face, 1.0);
    let cam = match above {
        true => Camera::new(Vec3::Y * 3.0, -90.0, -89.0),
        false => Camera::new(Vec3::NEG_Y * 3.0, -90.0, 89.0),
    };
    (scene, cam)
}

#[test]
fn gpu_a_both_sided_plane_is_lit_from_behind_and_a_front_one_is_culled() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut shade = |face, above, up| {
        let (scene, cam) = card(face, above, up);
        mean(&frame(&mut r, &scene, &cam))
    };
    let front_lit = shade(RenderFace::Front, true, false);
    let back_lit = shade(RenderFace::Both, false, true);
    let back_unlit = shade(RenderFace::Both, false, false);
    let culled = shade(RenderFace::Front, false, true);
    let back_only = shade(RenderFace::Back, false, true);
    let mut empty = Scene::new();
    (empty.skybox_path, empty.ambient_intensity) = (String::new(), 0.0);
    let sky = mean(&frame(
        &mut r,
        &empty,
        &card(RenderFace::Front, false, true).1,
    ));
    eprintln!(
        "[render face] front lit {front_lit:.1}; from behind: lit {back_lit:.1}, unlit \
         {back_unlit:.1}, Back {back_only:.1}; Front {culled:.1} vs no plane {sky:.1}"
    );
    assert!(
        back_lit > back_unlit + 30.0,
        "lit by the light on its seen side"
    );
    assert!((back_lit - front_lit).abs() < 8.0, "lit as its front is");
    assert!(
        (back_only - back_lit).abs() < 1.0,
        "Back draws the face Both does"
    );
    assert!(
        (culled - sky).abs() < 1.0,
        "Front culls the plane seen from behind"
    );
}
