//! A material's detail layer (#1997, HDRP's detail map), drawn: a layer without a
//! map or with a missing one draws what no layer draws, byte for byte; a busy map
//! changes a floor under each mapping and a neutral one (0.5) does not; and on a
//! coloured metal a darkening detail scales its reflection without tinting it.

use glam::Vec3;
use rusty::components::{DetailLayer, MaterialAsset, MaterialComponent, UvMapping};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, Scene};

/// An 8² detail map at a fresh path `name`: every texel `texel`, or seeded noise
/// in every channel when `None`.
fn detail_map(name: &str, texel: Option<[u8; 4]>) -> String {
    let mut state = 0x9e37_79b9u32;
    let img = image::RgbaImage::from_fn(8, 8, |_, _| {
        image::Rgba(texel.unwrap_or_else(|| {
            std::array::from_fn(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
        }))
    });
    let path = crate::temp::dir().join(name);
    img.save(&path).expect("write the detail map");
    path.to_string_lossy().into_owned()
}

/// `base` with a detail layer of `map` at its defaults, one repeat per 2 m under a
/// world mapping: coarse, so a 64×48 frame sees its texels.
fn with_detail(base: &MaterialAsset, map: Option<String>) -> MaterialAsset {
    let detail = DetailLayer {
        map: map.map(Into::into),
        world_size: 2.0,
        ..DetailLayer::default()
    };
    MaterialAsset {
        detail: Some(detail),
        ..base.clone()
    }
}

/// Each material's frame: a 6 m floor of it under the default sun and a grey sky,
/// seen from 2 m up, as RGB bytes. `None` without an adapter.
fn frames(materials: &[MaterialAsset]) -> Option<Vec<Vec<[u8; 3]>>> {
    let mut host = CaptureHost::new();
    let mut out = Vec::new();
    for (i, material) in materials.iter().enumerate() {
        let mut scene = Scene::new();
        let sky = crate::temp::dir().join("rusty_1997_sky.png");
        image::RgbaImage::from_pixel(2, 1, image::Rgba([120, 120, 120, 255]))
            .save(&sky)
            .expect("write the sky");
        scene.skybox_path = sky.to_string_lossy().into_owned();
        let id = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
        scene.world.transform_mut(id).unwrap().scale = Vec3::splat(0.6);
        scene
            .world
            .set_material(id, Some(MaterialComponent::new("floor")));
        scene.materials.insert("floor".into(), material.clone());
        let camera = Camera::new(Vec3::new(0.0, 2.0, 2.5), -90.0, -45.0);
        let path = crate::temp::dir().join(format!("rusty_1997_{i}.png"));
        if !capture_into(&mut host, &scene, &camera, &path, 64, 48).expect("capture") {
            eprintln!("[detail] no GPU/software adapter — skipping");
            return None;
        }
        let img = image::open(&path).expect("png").to_rgb8();
        out.push(img.pixels().map(|p| p.0).collect());
    }
    Some(out)
}

/// The mean absolute difference of two frames, per channel byte.
fn difference(a: &[[u8; 3]], b: &[[u8; 3]]) -> f64 {
    let lanes = a.iter().flatten().zip(b.iter().flatten());
    lanes.map(|(p, q)| f64::from(p.abs_diff(*q))).sum::<f64>() / (a.len() * 3) as f64
}

#[test]
fn a_layer_without_a_map_or_a_missing_one_draws_what_no_layer_draws() {
    let base = MaterialAsset {
        base_color: [0.6, 0.4, 0.3],
        ..MaterialAsset::default()
    };
    let mut mapless = with_detail(&base, None);
    mapless.detail.as_mut().unwrap().albedo_scale = 2.0;
    let missing = with_detail(&base, Some("rusty_1997_no_such_map.png".into()));
    if let Some(f) = frames(&[base, mapless, missing]) {
        assert!(f[0] == f[1] && f[0] == f[2], "no detail, no checker");
    }
}

#[test]
fn a_busy_detail_changes_each_mapping_and_a_neutral_one_does_not() {
    let busy = detail_map("rusty_1997_busy.png", None);
    let neutral = detail_map("rusty_1997_neutral.png", Some([128; 4]));
    for uv_mapping in [UvMapping::Uv0, UvMapping::Planar, UvMapping::Triplanar] {
        let base = MaterialAsset {
            base_color: [0.5, 0.5, 0.5],
            roughness: 0.4,
            uv_mapping,
            ..MaterialAsset::default()
        };
        let looks = [
            base.clone(),
            with_detail(&base, Some(busy.clone())),
            with_detail(&base, Some(neutral.clone())),
        ];
        let Some(f) = frames(&looks) else {
            return;
        };
        let (changed, kept) = (difference(&f[0], &f[1]), difference(&f[0], &f[2]));
        eprintln!("[detail] {uv_mapping:?}: busy {changed:.2}, neutral {kept:.2}");
        assert!(
            changed > 4.0,
            "{uv_mapping:?}: the busy detail shows ({changed:.2})"
        );
        assert!(kept < 0.5, "{uv_mapping:?}: 0.5 is neutral ({kept:.2})");
    }
}

#[test]
fn a_darkening_detail_scales_a_metals_reflection_without_tinting_it() {
    let gold = MaterialAsset {
        base_color: [1.0, 0.7, 0.3],
        metallic: 1.0,
        roughness: 0.15,
        ..MaterialAsset::default()
    };
    // R darkens the albedo; G, B and A are neutral: no bump, no roughness change.
    let dark = detail_map("rusty_1997_dark.png", Some([64, 128, 128, 128]));
    let Some(f) = frames(&[gold.clone(), with_detail(&gold, Some(dark))]) else {
        return;
    };
    let total = |frame: &[[u8; 3]], c: usize| frame.iter().map(|p| f64::from(p[c])).sum::<f64>();
    let ratio = |frame: &[[u8; 3]]| total(frame, 2) / total(frame, 0);
    let (plain, detailed) = (ratio(&f[0]), ratio(&f[1]));
    eprintln!("[detail] gold blue/red {plain:.3}, darkened {detailed:.3}");
    assert!(
        total(&f[1], 0) < 0.9 * total(&f[0], 0),
        "the detail darkens the metal"
    );
    assert!(
        (plain - detailed).abs() < 0.05,
        "{plain:.3} vs {detailed:.3}: no tint"
    );
}
