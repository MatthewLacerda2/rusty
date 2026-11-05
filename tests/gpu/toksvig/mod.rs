//! Toksvig specular AA (#1702): a glossy floor with a busy normal map widens its
//! lobe by what the map's mips lost, far off, and nothing else. Each test draws a
//! busy map against its *twin*: the same x and y, blue pinned at 255, so the shader
//! reads the same normals and no spread, as `main` did. No adapter: skip.

mod mapping;
mod shimmer;
mod surfaces;

use std::path::{Path, PathBuf};

use glam::Vec3;
use rusty::asset::sidecar::{set_texture, TextureSettings, TextureType};
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

pub(super) const W: u32 = 128;
pub(super) const H: u32 = 96;

/// Eye and pitch: far, 0.6 m up looking just below the horizon; near, 0.25 m up
/// looking down at the map's base level.
pub(super) const FAR: (Vec3, f32) = (Vec3::new(0.0, 0.6, 6.5), -4.0);
pub(super) const NEAR: (Vec3, f32) = (Vec3::new(0.0, 0.25, -2.5), -60.0);

/// What a map's blue holds: its normal's z (`Z`), or a fixed byte.
#[derive(Clone, Copy)]
pub(super) enum Blue {
    Z,
    Pinned(u8),
}

/// A 128² tangent-space normal map at a fresh path `name`: seeded noise, each
/// texel a whole normal tilted up to ~35°, or flat (x = y = 128) when `!busy`.
pub(super) fn normal_map(name: &str, busy: bool, blue: Blue) -> PathBuf {
    let mut state = 0x2545_f491u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        if busy {
            64 + (state >> 24) as u8 / 2
        } else {
            128
        }
    };
    let img = image::RgbaImage::from_fn(128, 128, |_, _| {
        let (x, y) = (next(), next());
        let (nx, ny) = (x as f32 / 127.5 - 1.0, y as f32 / 127.5 - 1.0);
        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
        let z = match blue {
            Blue::Z => (nz * 127.5 + 127.5).round() as u8,
            Blue::Pinned(b) => b,
        };
        image::Rgba([x, y, z, 255])
    });
    let path = crate::temp::dir().join(name);
    img.save(&path).expect("write the normal map");
    path
}

/// `map` typed `NormalMap` and imported: BC5, with its table, where the GPU has
/// BC; the route `Material.SetNormalMap` and a glTF import take.
pub(super) fn as_bc5(map: &Path) {
    let settings = TextureSettings {
        texture_type: TextureType::NormalMap,
        ..Default::default()
    };
    set_texture(map, settings).expect("type the normal map");
    rusty::asset::texture_import::refresh(map, "").expect("encode BC5");
}

/// A sky of one colour, `rgb`.
pub(super) fn sky(scene: &mut Scene, rgb: [u8; 3], name: &str) {
    let path = crate::temp::dir().join(name);
    let [r, g, b] = rgb;
    let img = image::RgbaImage::from_pixel(2, 1, image::Rgba([r, g, b, 255]));
    img.save(&path).expect("write the sky");
    scene.skybox_path = path.to_string_lossy().into_owned();
}

/// A black-sky scene with no ambient, and a 15 m floor of `material` (its map
/// tiled 8×) at y = `y`; returns the floor.
pub(super) fn floor(scene: &mut Scene, name: &str, y: f32, material: MaterialAsset) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Plane));
    scene.world.transform_mut(id).unwrap().position.y = y;
    let key = format!("{name}_m");
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(key.clone())));
    let tiled = MaterialAsset {
        tiling: [8.0, 8.0],
        ..material
    };
    scene.materials.insert(key, tiled);
    id
}

/// A glossy material of `metallic` and `roughness` with normal map `map`.
pub(super) fn glossy(map: &Path, metallic: f32, roughness: f32) -> MaterialAsset {
    MaterialAsset {
        metallic,
        roughness,
        normal_map: Some(map.to_string_lossy().into_owned().into()),
        ..MaterialAsset::default()
    }
}

/// Each of `frames` frames of `scene` from `view`, turning 0.4° a frame, as RGB
/// floats; `None` without an adapter.
pub(super) fn frames(scene: &Scene, view: (Vec3, f32), frames: u32) -> Option<Vec<Vec<[f64; 3]>>> {
    let mut host = CaptureHost::new();
    let mut out = Vec::new();
    for f in 0..frames {
        let camera = Camera::new(view.0, -90.0 + f as f32 * 0.4, view.1);
        let path = crate::temp::dir().join(format!("rusty_1702_{}_{f}.png", view.0.z));
        if !capture_into(&mut host, scene, &camera, &path, W, H).expect("capture") {
            eprintln!("[toksvig] no GPU/software adapter — skipping");
            return None;
        }
        let img = image::open(&path).expect("png").to_rgb8();
        out.push(img.pixels().map(|p| p.0.map(f64::from)).collect());
    }
    Some(out)
}

/// Each frame's luma.
pub(super) fn luma(frames: &[Vec<[f64; 3]>]) -> Vec<Vec<f64>> {
    let l = |p: &[f64; 3]| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
    frames.iter().map(|f| f.iter().map(l).collect()).collect()
}

/// The mean of every value.
pub(super) fn mean(values: &[Vec<f64>]) -> f64 {
    let n = values.iter().map(Vec::len).sum::<usize>() as f64;
    values.iter().flatten().sum::<f64>() / n
}
