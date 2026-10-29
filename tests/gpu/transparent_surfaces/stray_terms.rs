//! Surface terms that must stay where they belong (#1523): an unlit transparent
//! (a lamp's glass) blends straight, as before #1523, and a blend mode left on an
//! Opaque material blends nothing.

use glam::Vec3;
use rusty::components::{BlendMode, MaterialAsset};
use rusty::scene::{Camera, Scene};

use super::{differing, glass, half_alpha, mean_in, shot, slab, stage, SIZE};

/// A black wall, and in front of it, when `alpha` is some, a grey unlit
/// transparent pane at that alpha: unlit because it carries a light, as a lamp's
/// mesh does (a light of no intensity, so it lights nothing).
fn lamp_glass(alpha: Option<f32>) -> Scene {
    let mut scene = stage(-1.2);
    let wall = MaterialAsset {
        base_color: [0.0; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Wall",
        Vec3::new(0.0, 0.0, -2.0),
        Vec3::new(20.0, 20.0, 0.1),
        wall,
    );
    if let Some(alpha) = alpha {
        let pane = slab(
            &mut scene,
            "Lamp",
            Vec3::ZERO,
            Vec3::new(20.0, 20.0, 0.01),
            glass([0.5; 3], alpha),
        );
        let light = rusty::components::LightComponent {
            intensity: 0.0,
            ..rusty::scene::authoring::default_light()
        };
        scene.world.set_light(pane, Some(light));
    }
    scene
}

#[test]
fn an_unlit_transparent_blends_straight_over_what_is_behind() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let Some(wall) = shot(&lamp_glass(None), &cam, "unlit_wall") else {
        return;
    };
    let faint = shot(&lamp_glass(Some(0.3)), &cam, "unlit_faint").unwrap();
    let solid = shot(&lamp_glass(Some(1.0)), &cam, "unlit_solid").unwrap();
    let window = (SIZE / 4, SIZE * 3 / 4);
    let mean = |img| mean_in(img, window, window);
    let (wall, faint, solid) = (mean(&wall), mean(&faint), mean(&solid));
    eprintln!("[1523] unlit pane: wall {wall:.1}, alpha 0.3 {faint:.1}, alpha 1 {solid:.1}");
    // Straight alpha leaves 30% of a linear 0.5 over black, about 70% of the
    // solid pane's sRGB value; premultiplied, it would be the solid pane whole.
    assert!(
        faint > wall + 10.0,
        "the pane shows: {faint:.1} vs {wall:.1}"
    );
    assert!(
        faint - wall < 0.85 * (solid - wall),
        "an unlit pane at alpha 0.3 blends at 30%: {faint:.1} vs solid {solid:.1}"
    );
}

/// A lit Opaque box under the sun whose albedo map has half alpha, with `blend`.
fn opaque_box(blend: BlendMode) -> Scene {
    let mut scene = stage(-1.0);
    let material = MaterialAsset {
        base_color_map: Some(half_alpha("stray_half_alpha").into()),
        blend_mode: blend,
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Box",
        Vec3::ZERO,
        Vec3::new(20.0, 0.1, 20.0),
        material,
    );
    scene
}

#[test]
fn a_blend_mode_left_on_an_opaque_material_changes_nothing() {
    let cam = Camera::new(Vec3::new(0.0, 4.0, 4.0), -90.0, -45.0);
    let (Some(alpha), Some(multiply)) = (
        shot(&opaque_box(BlendMode::Alpha), &cam, "stray_alpha"),
        shot(&opaque_box(BlendMode::Multiply), &cam, "stray_multiply"),
    ) else {
        return;
    };
    let changed = differing(&alpha, &multiply);
    assert_eq!(
        changed, 0,
        "a leftover Multiply whitens an Opaque albedo: {changed} px"
    );
}
