//! #1995: an object between two blended box probes reflects them without a seam.
//! A curved glossy floor (a flattened sphere) straddles two probe boxes that meet at
//! x = 0, so half of it lies outside each of its two probes' boxes, the issue's
//! wedge room on primitives. Both probes hold one smooth cube (its colour is its
//! direction), so only a jump in how a probe is projected can draw a step. Before
//! #1995 a fragment past a box face fell to infinite projection: a hard straight
//! step down the floor at x = 0. Now it projects against the box, continuously, and
//! so does a right box turned like a wedge's slice (an oriented box, #1995).
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use crate::gpu::reflection_probe_per_object_screenshot::cube_ktx2;
use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

const CUBE: u32 = 32;

/// Face `f`'s texels: the linear colour `0.5 + 0.5 * dir` of each texel's
/// direction (the Vulkan cube face layout), continuous across faces.
fn gradient_face(f: usize) -> Vec<Vec3> {
    let n = CUBE as f32;
    (0..CUBE * CUBE)
        .map(|i| {
            let u = 2.0 * ((i % CUBE) as f32 + 0.5) / n - 1.0;
            let v = 2.0 * ((i / CUBE) as f32 + 0.5) / n - 1.0;
            let dir = match f {
                0 => Vec3::new(1.0, -v, -u),
                1 => Vec3::new(-1.0, -v, u),
                2 => Vec3::new(u, 1.0, v),
                3 => Vec3::new(u, -1.0, -v),
                4 => Vec3::new(u, -v, 1.0),
                _ => Vec3::new(-u, -v, -1.0),
            };
            Vec3::splat(0.5) + 0.5 * dir.normalize()
        })
        .collect()
}

/// The floor and the two probes, left box x in [-8, 0] and right box [0, 8], the
/// right one turned `turn` radians about Y.
fn scene(turn: f32) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let glossy = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness: 0.05,
        ..MaterialAsset::default()
    };
    scene.materials.insert("glossy".to_string(), glossy);
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Sphere));
    {
        let mut t = scene.world.transform_mut(floor).unwrap();
        (t.position, t.scale) = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(16.0, 1.0, 16.0));
    }
    let material = MaterialComponent::new("glossy".to_string());
    scene.world.set_material(floor, Some(material));
    let cube = crate::temp::dir().join("rusty_gpu_1995_gradient.ktx2");
    std::fs::write(&cube, cube_ktx2(CUBE, gradient_face)).unwrap();
    for side in [-1.0f32, 1.0] {
        let probes = &mut scene.reflection_probes;
        let i = probes.add(Vec3::new(4.0 * side, 1.5, 0.0), Vec3::ONE);
        let (lo, hi) = (8.0 * side.min(0.0), 8.0 * side.max(0.0));
        probes.set_box(i, Vec3::new(lo, -1.0, -8.0), Vec3::new(hi, 3.0, 8.0));
        probes.set_cubemap(i, cube.to_string_lossy().into_owned());
        if side > 0.0 {
            probes.edit(i, |p| p.rotation = glam::Quat::from_rotation_y(turn));
        }
    }
    scene
}

/// The largest jump between two horizontally adjacent pixels (summed RGB) in the
/// middle of the frame, where only the floor is seen.
fn largest_step(img: &image::RgbImage) -> i32 {
    let (w, h) = (img.width(), img.height());
    let mut worst = 0;
    for y in h * 3 / 10..h * 9 / 10 {
        for x in w / 5..w * 4 / 5 {
            let (a, b) = (img.get_pixel(x, y), img.get_pixel(x + 1, y));
            let d: i32 = (0..3).map(|c| (a[c] as i32 - b[c] as i32).abs()).sum();
            worst = worst.max(d);
        }
    }
    worst
}

#[test]
fn a_floor_between_two_box_probes_shows_no_step_at_their_shared_face() {
    let cam = Camera::new(Vec3::new(0.0, 3.0, 4.0), -90.0, -50.0);
    let mut host = CaptureHost::new();
    // Before #1995 the world-aligned pair stepped by 84 at x = 0 on lavapipe; the
    // smooth cube's own gradient and dither stay under 16.
    for turn in [0.0, 0.5] {
        let path = crate::temp::dir().join(format!("rusty_gpu_1995_seam_{turn}.png"));
        if !capture_into(&mut host, &scene(turn), &cam, &path, 160, 96).expect("capture") {
            eprintln!("[probes] no GPU/software adapter — skipping visual assertion");
            return;
        }
        let step = largest_step(&image::open(&path).expect("png").to_rgb8());
        eprintln!(
            "[probes] turn {turn}: largest step {step} ({})",
            path.display()
        );
        assert!(
            step < 36,
            "turn {turn}: a seam across the floor, step {step}"
        );
    }
}
