//! Reflection probes are picked per object (#1206), Unity's way: two adjacent probe
//! boxes, a red cube in the left one and a green cube in the right, a metal sphere in
//! each, and the camera inside the left box. Each sphere must reflect its own probe.
//! When the probe was picked once per camera, both spheres reflected the left (red)
//! one. No lights and no panorama, so the probes are all that colour the spheres.

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

/// A one-mip `size`² RGBA8 sRGB KTX2 cubemap, every face the solid `rgb`.
fn solid_cube_ktx2(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    const MAGIC: [u8; 12] = [
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let texels = (size * size * 6) as usize;
    let (header, index, dfd) = (80usize, 24usize, 4usize);
    let level = header + index + dfd;
    let mut buf = vec![0u8; level];
    buf[..12].copy_from_slice(&MAGIC);
    let put = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    // vkFormat R8G8B8A8_SRGB, typeSize, width, height, depth, layers, faces, levels.
    for (at, v) in [(12, 43), (16, 4), (20, size), (24, size), (36, 6), (40, 1)] {
        put(&mut buf, at, v);
    }
    put(&mut buf, 48, (header + index) as u32);
    put(&mut buf, 52, dfd as u32);
    let len = (texels * 4) as u64;
    for (k, v) in [level as u64, len, len].into_iter().enumerate() {
        buf[header + 8 * k..header + 8 * k + 8].copy_from_slice(&v.to_le_bytes());
    }
    for _ in 0..texels {
        buf.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    buf
}

/// Two probe boxes meeting at x = 0, each with a polished metal sphere at its centre.
fn scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_color = Vec3::splat(0.05);
    scene.ambient_intensity = 0.1;
    let metal = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness: 0.2,
        ..MaterialAsset::default()
    };
    scene.materials.insert("metal".to_string(), metal);
    for (x, rgb, name) in [(-2.0, [255, 0, 0], "red"), (2.0, [0, 255, 0], "green")] {
        let path = crate::temp::dir().join(format!("rusty_probe_{name}.ktx2"));
        std::fs::write(&path, solid_cube_ktx2(4, rgb)).expect("write the cube");
        let probes = &mut scene.reflection_probes;
        let i = probes.add(Vec3::new(x, 0.0, 0.0), Vec3::ONE);
        let (lo, hi) = if x < 0.0 { (-4.0, 0.0) } else { (0.0, 4.0) };
        probes.set_box(i, Vec3::new(lo, -4.0, -4.0), Vec3::new(hi, 4.0, 12.0));
        probes.set_cubemap(i, path.to_string_lossy().into_owned());
        let id = create_entity(&mut scene, name, Some(Primitive::Sphere));
        {
            let mut transform = scene.world.transform_mut(id).unwrap();
            transform.position = Vec3::new(x, 0.0, 0.0);
            transform.scale = Vec3::splat(2.0);
        }
        let material = MaterialComponent {
            material: "metal".to_string(),
        };
        scene.world.set_material(id, Some(material));
    }
    scene
}

/// Mean (red, green) of the 12x12 pixels around `(x, y)`.
fn red_green(img: &image::RgbImage, x: u32, y: u32) -> (f64, f64) {
    let (mut r, mut g) = (0.0, 0.0);
    for py in y - 6..y + 6 {
        for px in x - 6..x + 6 {
            let p = img.get_pixel(px, py);
            (r, g) = (r + p[0] as f64, g + p[1] as f64);
        }
    }
    (r / 144.0, g / 144.0)
}

#[test]
fn each_object_reflects_the_probe_whose_box_holds_it() {
    // Inside the left box, looking down -Z at both spheres.
    let cam = Camera::new(Vec3::new(-0.5, 0.0, 8.0), -90.0, 0.0);
    let path = crate::temp::dir().join("rusty_probe_per_object.png");
    let mut host = CaptureHost::new();
    if !capture_into(&mut host, &scene(), &cam, &path, 160, 96).expect("capture") {
        eprintln!("[probes] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    // The camera is 1.5 right of the red sphere and 2.5 left of the green one, so
    // the red sphere lands left of the image centre and the green one right of it.
    let (w, h) = (img.width(), img.height());
    let left = red_green(&img, w / 2 - w / 6, h / 2);
    let right = red_green(&img, w / 2 + w / 4, h / 2);
    eprintln!("[probes] left (r, g) = {left:?}, right (r, g) = {right:?}");
    assert!(
        left.0 > left.1 + 20.0,
        "left sphere must reflect red: {left:?}"
    );
    assert!(
        right.1 > right.0 + 20.0,
        "right sphere must reflect green: {right:?}"
    );
}
