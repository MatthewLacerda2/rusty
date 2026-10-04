//! Baked lightmaps on the GPU (#438). Two proofs:
//!
//! * **Bounce only.** A static floor and wall with zero ambient and one `Mixed` spot
//!   aimed at the wall: no light reaches the floor directly (it is outside the cone),
//!   so before the bake it renders black. After `bake_scene_lightmaps`, the floor's
//!   lightmap carries the wall's bounce and the same view renders visibly lit.
//! * **No lightmap UV, no change.** A Sphere has no second UV map, so no lightmap:
//!   its pixels render exactly as before the bake.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightMode, LightType};
use rusty::dev::lightmap_bake::bake_scene_lightmaps;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::lightmap::BakeSettings;
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

/// Add a static primitive at `position` scaled by `scale`; returns its id.
fn add(scene: &mut Scene, primitive: Primitive, position: Vec3, scale: Vec3) -> u32 {
    let id = scene.add_entity(format!("{primitive:?}"));
    scene
        .world
        .set_mesh(id, primitive_mesh_component(primitive));
    scene.world.set_static(id, true);
    let mut t = scene.world.transform_mut(id).unwrap();
    t.position = position;
    t.scale = scale;
    id
}

/// Floor, wall (x = 3), sphere (x = -3), and a `Mixed` 20° spot aimed at the wall.
fn scene(ambient: f32) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = ambient;
    for (primitive, at, scale) in [
        (Primitive::Plane, Vec3::ZERO, Vec3::ONE),
        (
            Primitive::Box,
            Vec3::new(3.25, 2.0, 0.0),
            Vec3::new(0.5, 4.0, 8.0),
        ),
        (Primitive::Sphere, Vec3::new(-3.0, 1.0, 0.0), Vec3::ONE),
    ] {
        add(&mut scene, primitive, at, scale);
    }
    let spot = scene.add_entity("Spot".to_string());
    scene.world.set_light(
        spot,
        Some(LightComponent {
            light_type: LightType::Spotlight,
            color: Vec3::ONE,
            intensity: 40.0,
            range: 20.0,
            inner_cone: 15.0,
            outer_cone: 20.0,
            cast_shadows: false,
            mode: LightMode::Mixed,
        }),
    );
    let mut t = scene.world.transform_mut(spot).unwrap();
    t.position = Vec3::new(0.0, 2.0, 0.0);
    t.rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
    drop(t);
    scene
}

/// Bake `scene`'s lightmaps into a temp folder named `name`.
fn bake(scene: &mut Scene, name: &str) {
    let dir = std::env::temp_dir().join(format!("rusty_gpu_lightmap_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 32,
        bounces: 2,
        seed: 3,
        max_resolution: 32,
        filter_radius: 2,
    };
    let baked = bake_scene_lightmaps(scene, Some(&path), &settings).unwrap();
    assert_eq!(
        baked, 2,
        "the floor and the wall; the sphere has no lightmap UV"
    );
}

/// `scene` seen from `camera`; `None` without an adapter.
fn shot(scene: &Scene, camera: &Camera, name: &str) -> Option<image::RgbImage> {
    let path = std::env::temp_dir().join(format!("rusty_gpu_lightmap_{name}.png"));
    capture(scene, camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).unwrap().to_rgb8())
}

fn mean(img: &image::RgbImage) -> f64 {
    let sum: u64 = img.pixels().flat_map(|p| p.0).map(u64::from).sum();
    sum as f64 / (img.pixels().len() * 3) as f64
}

#[test]
fn a_lightmapped_floor_is_lit_where_only_bounce_reaches_it() {
    // Down onto the floor by the wall, out of the spot's cone: only the sky's faint
    // reflection (environment reflections are always on) shows before the bake.
    let camera = Camera::new(Vec3::new(1.5, 2.5, 0.0), 0.0, -89.0);
    let mut lit = scene(0.0);
    let Some(before) = shot(&lit, &camera, "bounce_before") else {
        eprintln!("[lightmap] no GPU/software adapter — skipping");
        return;
    };
    bake(&mut lit, "bounce");
    let after = shot(&lit, &camera, "bounce_after").unwrap();
    let (before, after) = (mean(&before), mean(&after));
    eprintln!("[lightmap] floor before={before:.2} after={after:.2}");
    assert!(
        before < 10.0,
        "no direct light reaches the floor: {before:.2}"
    );
    assert!(
        after > before + 30.0,
        "the baked bounce lights it: {after:.2}"
    );
}

#[test]
fn a_mesh_without_lightmap_uvs_renders_as_before_the_bake() {
    // Down onto the sphere, under a lit ambient so it shows something to compare.
    let camera = Camera::new(Vec3::new(-3.0, 3.2, 0.0), 0.0, -89.0);
    let mut scene = scene(0.6);
    let Some(before) = shot(&scene, &camera, "fallback_before") else {
        return;
    };
    bake(&mut scene, "fallback");
    let after = shot(&scene, &camera, "fallback_after").unwrap();
    // The sphere fills the middle of the frame; compare its centre block exactly.
    let centre = |img: &image::RgbImage| -> Vec<[u8; 3]> {
        let (lo, hi) = (SIZE / 2 - 6, SIZE / 2 + 6);
        (lo..hi)
            .flat_map(|y| (lo..hi).map(move |x| (x, y)))
            .map(|(x, y)| img.get_pixel(x, y).0)
            .collect()
    };
    assert!(mean(&before) > 10.0, "the ambient lights the sphere");
    assert_eq!(centre(&before), centre(&after));
}
