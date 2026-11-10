//! The baked lobe against the realtime truth, a rough plane it leaves alone, and the
//! `HalfLambert` diffuse model, whose squared wrap the lobe divides out.

use glam::Vec3;
use rusty::components::LightMode;
use rusty::scene::lighting::DiffuseModel;
use rusty::scene::{Camera, Scene};

use super::{bake, most, peak, scene, shot};

/// A camera 2.5 m off along -X, 1.5 m up, looking at the lamp's foot: the lamp's
/// mirror image sits 1 m short of it.
fn camera() -> Camera {
    Camera::new(Vec3::new(-2.5, 1.5, 0.0), 0.0, -31.0)
}

/// `scene` baked (`directional` or not) under `name`, then shot.
fn baked(mut scene: Scene, name: &str, directional: bool) -> Option<Vec<f32>> {
    bake(&mut scene, name, directional, super::DENSITY);
    shot(&scene, &camera(), name)
}

/// The baked lobe's peak against the live lamp's, over the same bake without
/// directions: (baked x, y, peak), (live x, y, peak).
pub(super) fn highlights(
    make: impl Fn(LightMode) -> Scene,
    tag: &str,
) -> Option<[(u32, u32, f32); 2]> {
    let lobed = baked(make(LightMode::Baked), &format!("{tag}_lobed"), true)?;
    let diffuse = baked(make(LightMode::Baked), &format!("{tag}_diffuse"), false).unwrap();
    let live = baked(make(LightMode::Realtime), &format!("{tag}_live"), false).unwrap();
    let pair = [peak(&lobed, &diffuse), peak(&live, &diffuse)];
    eprintln!(
        "[baked-spec] {tag}: baked {:?}, live {:?}",
        pair[0], pair[1]
    );
    Some(pair)
}

/// The two peaks sit within `px` pixels of each other and within 1.5x in height.
pub(super) fn agree([(bx, by, b), (lx, ly, l)]: [(u32, u32, f32); 2], px: u32) {
    assert!(l > 20.0, "the live lamp shows a highlight: {l:.1}");
    assert!(
        bx.abs_diff(lx) <= px && by.abs_diff(ly) <= px,
        "the baked highlight sits where the live one does"
    );
    let ratio = b / l;
    assert!(
        (1.0 / 1.5..1.5).contains(&ratio),
        "the baked peak is within 1.5x of the live one: {ratio:.2}"
    );
}

#[test]
fn a_baked_lamps_highlight_sits_where_its_realtime_one_does() {
    let Some(pair) = highlights(|mode| scene(mode, 0.25), "glossy") else {
        eprintln!("[baked-spec] no GPU/software adapter — skipping");
        return;
    };
    agree(pair, 2);
}

#[test]
fn half_lambert_keeps_the_highlight_its_live_lamp_has() {
    let make = |mode| {
        let mut s = scene(mode, 0.25);
        s.lighting_settings.diffuse_model = DiffuseModel::HalfLambert;
        s
    };
    if let Some(pair) = highlights(make, "wrap") {
        agree(pair, 2);
    }
}

#[test]
fn a_rough_plane_shows_no_visible_change() {
    let Some(flat) = baked(scene(LightMode::Baked, 1.0), "rough_flat", false) else {
        return;
    };
    let lobed = baked(scene(LightMode::Baked, 1.0), "rough_lobed", true).unwrap();
    let (_, _, most_brighter) = peak(&lobed, &flat);
    eprintln!("[baked-spec] rough: at most {most_brighter:.1} brighter");
    assert!(
        most_brighter <= 4.0,
        "roughness 1 spreads the lobe to nothing: {most_brighter:.1}"
    );
    assert!(most(&lobed, &flat) <= 4.0, "nor darkens anything");
}
