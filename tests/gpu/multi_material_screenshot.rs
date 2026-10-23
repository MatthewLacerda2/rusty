//! A mesh with two materials draws each run with its own (#1285): the FBX fixture's
//! quad is a `Red` triangle and a `Blue` one, and both show. Pointing slot 1 at a
//! third material changes only the triangle that slot draws.
//!
//! Each material is made flat emissive over a black base in a scene with no ambient
//! and no lights, so a pixel's colour can only come from the material its run drew
//! with. Skips without an adapter, like every capture test.

use crate::temp::{dir, file as tmp};
use glam::Vec3;
use rusty::components::MaterialAsset;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{instantiate_asset, material as mat_ops};
use rusty::scene::{Camera, Scene};

/// Pixels whose colour is dominated by red, green and blue.
fn dominant(path: &std::path::Path) -> [usize; 3] {
    let img = image::open(path).expect("open the capture").to_rgb8();
    let mut counts = [0; 3];
    for p in img.pixels() {
        let [r, g, b] = p.0.map(i32::from);
        for (i, (own, others)) in [(r, g.max(b)), (g, r.max(b)), (b, r.max(g))]
            .into_iter()
            .enumerate()
        {
            if own > 80 && own > others * 3 {
                counts[i] += 1;
            }
        }
    }
    counts
}

/// A flat emissive material of colour `rgb`: lit by nothing else.
fn glow(rgb: [f32; 3]) -> MaterialAsset {
    MaterialAsset {
        base_color: [0.0; 3],
        emissive: rgb,
        ..MaterialAsset::default()
    }
}

/// The fixture quad, grown to two metres, its two materials made to glow red and
/// blue, plus a `green` one; and a camera facing its front.
fn quad_scene() -> (Scene, u32, Camera) {
    let path = dir().join("quad.fbx");
    std::fs::write(
        &path,
        include_str!("../fixtures/models/embedded_textures.fbx"),
    )
    .unwrap();
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    let reference = format!("{}::Quad", path.to_string_lossy());
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    let keys = mat_ops::keys(&scene.world, id);
    assert_eq!(keys.len(), 2, "one material per run: {keys:?}");
    for (key, rgb) in keys.iter().zip([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]) {
        scene.materials.insert(key.clone(), glow(rgb));
    }
    scene
        .materials
        .insert("green".into(), glow([0.0, 1.0, 0.0]));

    let (min, max) = scene
        .world
        .mesh(id)
        .unwrap()
        .world_aabb(glam::Mat4::IDENTITY)
        .unwrap();
    let scale = 2.0 / (max - min).max_element();
    let centre = (min + max) * 0.5 * scale;
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(scale);
    let normal = Vec3::from(scene.world.mesh(id).unwrap().vertices[0].normal);
    let look = -normal.normalize();
    let cam = Camera::new(
        centre - look * 3.0,
        look.z.atan2(look.x).to_degrees(),
        look.y.asin().to_degrees(),
    );
    (scene, id, cam)
}

#[test]
fn each_material_run_draws_with_its_own_material() {
    let (mut scene, id, cam) = quad_scene();
    let (both, swapped) = (
        tmp("rusty_two_materials.png"),
        tmp("rusty_slot_swapped.png"),
    );
    if !capture(&scene, &cam, &both, 96, 96).expect("capture") {
        eprintln!("[multi-material] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let [red, green, blue] = dominant(&both);
    eprintln!("[multi-material] red={red} green={green} blue={blue}");
    assert!(
        red > 300 && blue > 300,
        "both halves show: red={red} blue={blue}"
    );
    assert_eq!(green, 0);

    let materials = scene.materials.clone();
    mat_ops::set_slot(&mut scene.world, &materials, id, 1, "green").unwrap();
    capture(&scene, &cam, &swapped, 96, 96).expect("capture");
    let [red2, green2, blue2] = dominant(&swapped);
    eprintln!("[multi-material] after SetAt: red={red2} green={green2} blue={blue2}");
    assert_eq!(red2, red, "slot 0's triangle is untouched");
    assert!(green2 > 300 && blue2 == 0, "slot 1's triangle is now green");
}
