//! The height map under `Planar`: the projection runs along world
//! Y, so on a vertical face one UV axis never changes and one repeat stretches
//! across the face. That face must draw as `Planar` does without a height map
//! (stretched), not as a march along the stretch (noise); a floor still marches.
//! The box leans 10°, so its side is steep but not exactly vertical: the face
//! the review saw draw as noise.

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent, UvMapping};
use rusty::edit::authoring::{create_entity, Primitive};
use rusty::scene::Camera;

use super::{grazing, SIZE};
use crate::gpu::linear_data_maps_screenshot::{diff, SAME};
use crate::gpu::normal_emissive_maps_screenshot::scene;

/// An 8×8 black-and-white checker PNG, so a projection reads stripes along
/// either axis.
fn checker() -> String {
    let path = crate::temp::dir().join("rusty_2191_checker.png");
    let img = image::RgbaImage::from_fn(64, 64, |x, y| match (x / 8 + y / 8) % 2 {
        0 => image::Rgba([255, 255, 255, 255]),
        _ => image::Rgba([0, 0, 0, 255]),
    });
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

/// The checker as albedo under `Planar` (4 m a repeat), with `scale` as height.
fn planar(scale: Option<f32>) -> MaterialAsset {
    let map = checker();
    MaterialAsset {
        base_color_map: Some(map.clone().into()),
        roughness: 1.0,
        uv_mapping: UvMapping::Planar,
        world_size: 4.0,
        height_map: scale.map(|_| map.into()),
        height_scale: scale.unwrap_or(0.0),
        ..MaterialAsset::default()
    }
}

/// `material` on the 16 m floor or, when `wall`, on an 8 m box in its place
/// whose -X side faces the camera.
fn frame(material: MaterialAsset, wall: bool, cam: &Camera, name: &str) -> Option<image::RgbImage> {
    let mut s = scene(material);
    if wall {
        let floor = s.find_entity_by_name("Plane").expect("the floor");
        s.world.set_mesh(floor, None);
        let id = create_entity(&mut s, "Box", Some(Primitive::Box));
        let mut t = s.world.transform_mut(id).expect("its transform");
        (t.position, t.scale) = (Vec3::new(4.0, 0.0, 0.0), Vec3::splat(8.0));
        t.rotation = glam::Quat::from_rotation_z(10f32.to_radians());
        drop(t);
        s.world
            .set_material(id, Some(MaterialComponent::new("mat")));
    }
    let path = crate::temp::dir().join(name);
    rusty::dev::screenshot::capture(&s, cam, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).expect("open").to_rgb8())
}

#[test]
fn a_planar_wall_draws_stretched_and_a_planar_floor_still_marches() {
    // 15° above the wall's normal, from 7 m.
    let wall_cam = Camera::new(
        Vec3::new(-7.0, 7.0 * 15f32.to_radians().tan(), 0.0),
        0.0,
        -15.0,
    );
    let frames = [
        frame(planar(None), true, &wall_cam, "rusty_2191_wall_flat.png"),
        frame(
            planar(Some(0.1)),
            true,
            &wall_cam,
            "rusty_2191_wall_pom.png",
        ),
        frame(
            planar(None),
            false,
            &grazing(15.0),
            "rusty_2191_floor_flat.png",
        ),
        frame(
            planar(Some(0.1)),
            false,
            &grazing(15.0),
            "rusty_2191_floor_pom.png",
        ),
    ];
    let [Some(wall_flat), Some(wall_pom), Some(floor_flat), Some(floor_pom)] = frames else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    let (wall, floor) = (diff(&wall_flat, &wall_pom), diff(&floor_flat, &floor_pom));
    eprintln!("[height-map] planar wall {wall:.3}, planar floor {floor:.3}");
    // Measured: the wall RTX 2060 0.000, lavapipe 0.545 (37.4 when the march ran
    // there); the floor RTX 2060 41.0, lavapipe 40.9.
    assert!(wall < SAME, "the wall draws as without the map: {wall:.3}");
    assert!(floor > SAME, "the floor is displaced: {floor:.3}");
}
