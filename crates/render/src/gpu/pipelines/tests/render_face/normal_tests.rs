use glam::{Quat, Vec3};

use super::render_face_tests::{asset, card, frame, mean};
use rusty_components::{Projection, RenderFace};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

fn tilted_map(name: &str, tilt: f32) -> String {
    let z = (1.0 - tilt * tilt).sqrt();
    let byte = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([128, byte(tilt), byte(z), 255]));
    let path =
        rusty_test_support::temp::dir().join(format!("rusty_{name}_{}.png", std::process::id()));
    img.save(&path).expect("normal map written");
    path.to_string_lossy().into_owned()
}

fn mapped_card(r: &mut crate::Renderer, map: &str, above: bool) -> f32 {
    let (mut scene, cam) = card(RenderFace::Both, above, !above);
    let id = scene.world.find_by_name("Card").expect("the card");
    asset(&mut scene, id).normal_map = Some(map.to_owned().into());
    mean(&frame(r, &scene, &cam))
}

#[test]
fn gpu_a_normal_mapped_card_reads_mirrored_from_behind() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(48, 48) else {
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

fn ortho_underside(r: &mut crate::Renderer, up: bool) -> f32 {
    let mut scene = Scene::new();
    (scene.skybox_path, scene.ambient_intensity) = (String::new(), 0.0);
    let sun =
        rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
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
    let Some(mut r) = crate::test_gpu::headless_or_skip(48, 48) else {
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
