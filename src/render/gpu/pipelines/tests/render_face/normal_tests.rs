//! A two-sided face seen from behind (#1615), beyond its flat normal: a normal map
//! reads mirrored through the surface (URP's *Mirror* mode: the tangent's
//! handedness turns with the normal), and an orthographic camera sees the side its
//! parallel rays hit, wherever the camera itself stands (#430). Skips with no
//! adapter.

use glam::{Quat, Vec3};

use super::render_face_tests::{asset, card, frame, mean};
use crate::components::{Projection, RenderFace};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A flat tangent-space normal map tilted `tilt` along the bitangent, written to disk.
fn tilted_map(name: &str, tilt: f32) -> String {
    let z = (1.0 - tilt * tilt).sqrt();
    let byte = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([128, byte(tilt), byte(z), 255]));
    let path = crate::test_temp::dir().join(format!("rusty_{name}_{}.png", std::process::id()));
    img.save(&path).expect("normal map written");
    path.to_string_lossy().into_owned()
}

/// The `card` (a `Both` plane under a slanted sun) normal-mapped by `map`: seen from
/// above under a sun shining down, or from below under its mirror image.
fn mapped_card(r: &mut crate::render::Renderer, map: &str, above: bool) -> f32 {
    let (mut scene, cam) = card(RenderFace::Both, above, !above);
    let id = scene.world.find_by_name("Card").expect("the card");
    asset(&mut scene, id).normal_map = Some(map.to_owned().into());
    mean(&frame(r, &scene, &cam))
}

#[test]
fn gpu_a_normal_mapped_card_reads_mirrored_from_behind() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(48, 48) else {
        return;
    };
    let (toward, away) = (tilted_map("tilt_pos", 0.6), tilted_map("tilt_neg", -0.6));
    let front = mapped_card(&mut r, &toward, true);
    let other = mapped_card(&mut r, &away, true);
    let behind = mapped_card(&mut r, &toward, false);
    eprintln!("[render face normals] front {front:.1}, tilted the other way {other:.1}, behind {behind:.1}");
    assert!((front - other).abs() > 10.0, "control: the tilt shows");
    assert!(
        (behind - front).abs() < 6.0,
        "from behind, under the mirrored sun, the map shades as from the front"
    );
}

/// A large `Both` plane seen through an orthographic camera standing above it, whose
/// rays climb and hit its underside; the sun shines `up` onto that side, or down.
fn ortho_underside(r: &mut crate::render::Renderer, up: bool) -> f32 {
    let mut scene = Scene::new();
    (scene.skybox_path, scene.ambient_intensity) = (String::new(), 0.0);
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let tilt = if up { 1.2 } else { -1.2 };
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(tilt);
    let id = create_entity(&mut scene, "Card", Some(Primitive::Plane));
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(4.0);
    let m = asset(&mut scene, id);
    (m.render_face, m.roughness) = (RenderFace::Both, 1.0);
    let mut cam = Camera::new(Vec3::new(0.0, 0.5, -10.0), 90.0, 10.0);
    cam.projection = Projection::Orthographic { size: 5.0 };
    mean(&frame(r, &scene, &cam))
}

#[test]
fn gpu_an_orthographic_view_shades_the_side_its_rays_hit() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(48, 48) else {
        return;
    };
    let (lit, unlit) = (
        ortho_underside(&mut r, true),
        ortho_underside(&mut r, false),
    );
    eprintln!("[render face ortho] underside under a sun shining up {lit:.1}, down {unlit:.1}");
    assert!(
        lit > unlit + 10.0,
        "the underside is shaded as the underside, though the camera stands above"
    );
}
