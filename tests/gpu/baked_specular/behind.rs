//! A two-sided surface seen from behind (#1695's review). The bake lights only the
//! front, so the back face shows the front's baked light, and its normal map and
//! geometric normal are mirrored through the surface. The directions' rebalance
//! mirrors the page's direction with them: a normal-mapped back face reads what the
//! non-directional bake gives, reshaped a little, never many times brighter or black.

use glam::{Quat, Vec3};
use rusty::components::{LightMode, MaterialAsset, RenderFace};
use rusty::scene::{Camera, Scene};

use super::{bake, lamp, plane, point, shot, SIZE};

/// A normal map whose low-U half tilts about 35° toward -tangent, the rest +tangent.
fn bumps(path: &std::path::Path) {
    let img = image::RgbaImage::from_fn(64, 64, |x, _| match x < 32 {
        true => image::Rgba([64, 128, 218, 255]),
        false => image::Rgba([192, 128, 218, 255]),
    });
    img.save(path).expect("write normal map");
}

/// A two-sided, glossy, normal-mapped static plane with a lamp a metre over its front.
fn scene(name: &str) -> Scene {
    let dir = crate::temp::dir().join(format!("rusty_gpu_baked_spec_{name}_map"));
    std::fs::create_dir_all(&dir).unwrap();
    let map = dir.join("bumps.png");
    bumps(&map);
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let material = MaterialAsset {
        base_color: [0.8; 3],
        roughness: 0.25,
        normal_map: Some(map.to_string_lossy().into_owned().into()),
        render_face: RenderFace::Both,
        ..MaterialAsset::default()
    };
    plane(
        &mut scene,
        "panel",
        Vec3::ZERO,
        Quat::IDENTITY,
        0.2,
        material,
    );
    lamp(&mut scene, Vec3::Y, Quat::IDENTITY, point(LightMode::Baked));
    scene
}

/// The panel baked (`directional` or not) and seen from 2 m under it, looking up.
fn from_behind(directional: bool) -> Option<Vec<f32>> {
    let name = format!("behind_{directional}");
    let mut scene = scene(&name);
    bake(&mut scene, &name, directional, super::DENSITY);
    shot(
        &scene,
        &Camera::new(Vec3::new(0.0, -2.0, 0.0), 0.0, 89.9),
        &name,
    )
}

/// The mean of the image's top and bottom thirds: the normal map's halves run along
/// the panel's X, which a view straight up shows vertically.
fn halves(img: &[f32]) -> [f32; 2] {
    let band = |ys: std::ops::Range<u32>| {
        let rows = &img[(ys.start * SIZE) as usize..(ys.end * SIZE) as usize];
        rows.iter().sum::<f32>() / rows.len() as f32
    };
    [band(0..SIZE / 3), band(SIZE - SIZE / 3..SIZE)]
}

#[test]
fn a_two_sided_back_face_reads_its_front_s_baked_light() {
    let Some(lobed) = from_behind(true) else {
        return;
    };
    let plain = halves(&from_behind(false).unwrap());
    let lobed = halves(&lobed);
    eprintln!("[baked-spec] behind: directional {lobed:?}, plain {plain:?}");
    for (d, p) in lobed.iter().zip(&plain) {
        assert!(*p > 10.0, "the front's baked light shows behind: {p:.1}");
        let ratio = d / p;
        assert!(
            (0.7..1.1).contains(&ratio),
            "reshaped a little, not many times over or black: {ratio:.2}"
        );
    }
}
