//! Normals go to world space by the inverse-transpose of the model matrix (#1596):
//! a sphere squashed to (4,1,1) and lit from straight above shades by its true,
//! mostly-upward normals, not the slanted ones the model matrix itself would give.
//! A mirrored (-4,1,1) one shades the same, its normals still outward. Skips
//! without an adapter.

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, default_light, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 128;

/// A matte grey unit sphere scaled by `scale`, lit only by a sun pointing down -Y.
fn scene(scale: Vec3) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let id = create_entity(&mut scene, "Ellipsoid", Some(Primitive::Sphere));
    scene.world.transform_mut(id).unwrap().scale = scale;
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("m".to_string())));
    let matte = MaterialAsset {
        base_color: [0.8, 0.8, 0.8],
        metallic: 0.0,
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), matte);
    let sun = scene.add_entity("Sun".to_string());
    let light = LightComponent {
        light_type: LightType::Directional,
        intensity: 1.0,
        ..default_light()
    };
    scene.world.set_light(sun, Some(light));
    // A light shines down its -Z; turned about X, -Z points down -Y.
    scene.world.transform_mut(sun).unwrap().rotation =
        Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    scene
}

/// Mean green of the 3x3 block around where `world` lands in `img`.
fn at(img: &image::RgbImage, cam: &Camera, world: Vec3) -> f64 {
    let clip = cam.build_view_projection(1.0) * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * SIZE as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * SIZE as f32) as u32;
    let mut sum = 0.0;
    for py in y - 1..=y + 1 {
        for px in x - 1..=x + 1 {
            sum += img.get_pixel(px, py)[1] as f64;
        }
    }
    sum / 9.0
}

/// The shade on the centre column against halfway out to each side, at the same
/// height up the ellipsoid. `None` when no adapter rendered.
fn shades(label: &str, scale: Vec3) -> Option<(f64, f64, f64)> {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_normal_matrix_{label}.png"));
    if !capture(&scene(scale), &cam, &path, SIZE, SIZE).expect("capture") {
        eprintln!("[normal_matrix] no GPU/software adapter — skipping");
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    // Points on the unit-radius-0.5 sphere, scaled: the centre column at local
    // (0, 0.6, 0.8) and halfway out at local (±0.5, 0.6, 0.62), both radius 0.5.
    let (sx, sy, sz) = (scale.x.abs() * 0.5, scale.y * 0.5, scale.z * 0.5);
    let centre = at(&img, &cam, Vec3::new(0.0, 0.6 * sy, 0.8 * sz));
    let left = at(&img, &cam, Vec3::new(-0.5 * sx, 0.6 * sy, 0.62 * sz));
    let right = at(&img, &cam, Vec3::new(0.5 * sx, 0.6 * sy, 0.62 * sz));
    eprintln!("[normal_matrix] {label}: left {left:.1} centre {centre:.1} right {right:.1}");
    assert!(
        centre > 40.0,
        "{label}: the lit top did not render ({centre:.1})"
    );
    Some((left, centre, right))
}

/// True normals there are ≈(0.14, 0.69, 0.71): as bright as the centre column's
/// (0, 0.6, 0.8) or brighter. The model matrix alone slants them to ≈(0.92, 0.28,
/// 0.29), well under half the centre's light.
fn assert_true_normals(label: &str, scale: Vec3) {
    let Some((left, centre, right)) = shades(label, scale) else {
        return;
    };
    for (side, shade) in [("left", left), ("right", right)] {
        assert!(
            shade > 0.9 * centre,
            "{label}: the {side} flank is lit as if its normal slanted ({shade:.1} vs centre {centre:.1})"
        );
    }
}

#[test]
fn a_squashed_sphere_lit_from_above_shades_by_its_true_normals() {
    assert_true_normals("squashed", Vec3::new(4.0, 1.0, 1.0));
}

#[test]
fn a_mirrored_squashed_sphere_keeps_its_normals_outward() {
    assert_true_normals("mirrored", Vec3::new(-4.0, 1.0, 1.0));
}
