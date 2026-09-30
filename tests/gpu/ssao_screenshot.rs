//! SSAO (#436): where a crate meets the floor the ambient light darkens; an open,
//! flat floor does not. Each scene renders twice — AO on and off — so the only
//! difference between the frames is the occlusion. Skips without an adapter.

use glam::Vec3;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, default_visual_correction, Primitive};
use rusty::scene::{Camera, Scene};

/// A floor slab, optionally with a crate standing on it, lit by ambient light alone
/// (no sun entity), so AO is the dominant change.
fn yard(with_crate: bool, ssao: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.6;
    let mut boxes = vec![(
        "Floor",
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(20.0, 1.0, 20.0),
    )];
    if with_crate {
        boxes.push(("Crate", Vec3::new(0.0, 1.0, 0.0), Vec3::splat(2.0)));
    }
    for (name, position, scale) in boxes {
        let id = create_entity(&mut scene, name, Some(Primitive::Box));
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = position;
        t.scale = scale;
    }
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_visual_correction();
    vc.bloom_active = false;
    vc.ssr_active = false;
    vc.ssao.active = ssao;
    scene.world.set_visual_correction(volume, Some(vc));
    scene
}

/// Pixels clearly darker in `on` than in `off`, or `None` when no adapter rendered.
fn darkened(with_crate: bool, tag: &str) -> Option<usize> {
    let cam = Camera::new(Vec3::new(0.0, 2.5, 5.0), -90.0, -20.0);
    let dir = std::env::temp_dir();
    let id = std::process::id();
    let on_path = dir.join(format!("rusty_ssao_{tag}_on_{id}.png"));
    let off_path = dir.join(format!("rusty_ssao_{tag}_off_{id}.png"));
    let on = capture(&yard(with_crate, true), &cam, &on_path, 96, 96).expect("capture");
    let off = capture(&yard(with_crate, false), &cam, &off_path, 96, 96).expect("capture");
    if !on || !off {
        return None;
    }
    let a = image::open(&on_path).expect("png readable").to_rgb8();
    let b = image::open(&off_path).expect("png readable").to_rgb8();
    let sum = |p: &image::Rgb<u8>| p.0.iter().map(|&c| c as i32).sum::<i32>();
    let count = a
        .pixels()
        .zip(b.pixels())
        .filter(|(on, off)| sum(off) - sum(on) > 15)
        .count();
    //DBG let _ = std::fs::remove_file(on_path);
    //DBG let _ = std::fs::remove_file(off_path);
    Some(count)
}

#[test]
fn a_crate_darkens_the_floor_where_it_stands() {
    let Some(count) = darkened(true, "crate") else {
        eprintln!("[ssao] no GPU/software adapter — skipping visual assertion");
        return;
    };
    eprintln!("[ssao] pixels darkened around the crate: {count}");
    // The contact band and the crate's lower faces cover well over this of 96².
    assert!(count > 60, "the crate's foot should be occluded: {count}");
}

#[test]
fn an_open_flat_floor_is_not_darkened() {
    let Some(count) = darkened(false, "floor") else {
        eprintln!("[ssao] no GPU/software adapter — skipping visual assertion");
        return;
    };
    eprintln!("[ssao] pixels darkened on the open floor: {count}");
    assert!(count <= 5, "a flat floor must not occlude itself: {count}");
}
