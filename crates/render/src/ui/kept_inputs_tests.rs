//! Each input of canvas A's mesh, changed in turn: A alone is rebuilt, to what a
//! fresh build makes (`Rig::frame` compares), and is still on the next frame.

use glam::{Vec2, Vec4};

use super::{element, hud, Hud, Rig};
use rusty_components::{
    CanvasGroupComponent, ImageComponent, ImageType, TextAlignment, TextComponent, TextOverflow,
};

/// What each step changes, by name.
const STEPS: [&str; 19] = [
    "a rect",
    "an image colour",
    "a fill",
    "a fill amount",
    "a nine-slice border",
    "a sprite",
    "a string",
    "a font size",
    "a text colour",
    "an alignment",
    "an overflow",
    "a canvas group",
    "a rect mask",
    "a shape",
    "deactivated",
    "reactivated",
    "an element added",
    "reparented",
    "an element removed",
];

fn edit(h: &mut Hud, step: usize) {
    let w = &mut h.scene.world;
    match step {
        0 => w.rect_transform_mut(h.panel).expect("rect").size_delta.x = 410.0,
        1 => w.image_mut(h.panel).expect("image").color.x = 0.5,
        2 => w.image_mut(h.panel).expect("image").image_type = ImageType::Filled,
        3 => w.image_mut(h.panel).expect("image").fill_amount = 0.25,
        4 => {
            let mut i = w.image_mut(h.panel).expect("image");
            (i.image_type, i.border) = (ImageType::Sliced, Vec4::splat(9.0));
        }
        5 => w.image_mut(h.panel).expect("image").texture = Some("other.png".into()),
        6 => w.text_mut(h.label).expect("text").text = "AMMO 29".into(),
        7 => w.text_mut(h.label).expect("text").font_size = 41.0,
        8 => w.text_mut(h.label).expect("text").color.y = 0.2,
        9 => w.text_mut(h.label).expect("text").alignment = TextAlignment::BottomRight,
        10 => w.text_mut(h.label).expect("text").overflow = TextOverflow::Truncate,
        11 => {
            let group = CanvasGroupComponent {
                alpha: 0.5,
                ..Default::default()
            };
            _ = w.set_canvas_group(h.panel, Some(group));
        }
        12 => _ = w.set_rect_mask(h.panel, Some(Default::default())),
        13 => _ = w.set_shape(h.label, Some(Default::default())),
        14 | 15 => _ = w.set_active(h.label, step == 15),
        16 => {
            let id = element(&mut h.scene, h.panel, Vec2::new(5.0, 5.0));
            _ = h.scene.world.set_image(id, Some(ImageComponent::default()));
        }
        17 => h
            .scene
            .set_parent(h.label, Some(h.a))
            .expect("parent exists"),
        _ => h.scene.destroy_entity(h.label),
    }
}

#[test]
fn every_input_of_a_canvas_rebuilds_it_and_only_it() {
    let mut hud = hud();
    let mut rig = Rig::new();
    rig.frame(&hud.scene);
    for (step, what) in STEPS.iter().enumerate() {
        edit(&mut hud, step);
        assert_eq!(rig.frame(&hud.scene), 1, "{what}: canvas A alone");
        assert_eq!(rig.frame(&hud.scene), 0, "{what}: then still");
    }
}

#[test]
fn a_texture_size_the_screen_an_atlas_and_a_new_world_rebuild() {
    let mut hud = hud();
    let mut rig = Rig::new();
    rig.frame(&hud.scene);
    rig.texture = Vec2::splat(64.0);
    assert_eq!(
        rig.frame(&hud.scene),
        1,
        "a texture of another size: its canvas"
    );
    rig.screen = Vec2::new(1280.0, 720.0);
    assert_eq!(rig.frame(&hud.scene), 2, "the screen size: every canvas");
    // A text under B packs enough big glyphs to grow the atlas A's label is in.
    let big = element(&mut hud.scene, hud.bar, Vec2::ZERO);
    let glyphs: String = ('!'..='~').chain('À'..='ÿ').collect();
    let text = TextComponent {
        text: glyphs,
        font_size: 120.0,
        wrap: true,
        ..Default::default()
    };
    hud.scene.world.set_text(big, Some(text));
    let generation = rig.atlases.generation;
    assert_eq!(
        rig.frame(&hud.scene),
        3,
        "B, then A and B again for their moved uvs"
    );
    assert!(rig.atlases.generation > generation, "the atlas grew");
    assert_eq!(rig.frame(&hud.scene), 0);
    hud.scene.world.rewind_changes();
    assert_eq!(rig.frame(&hud.scene), 2, "an Undo: every canvas");
    // A canvas the view stops drawing drops its mesh, so a write meanwhile is seen.
    rig.skip = Some(hud.a);
    assert_eq!(rig.frame(&hud.scene), 0);
    hud.scene.world.text_mut(hud.label).expect("text").text = "AMMO 1".into();
    assert_eq!(rig.frame(&hud.scene), 0, "A is not drawn");
    rig.skip = None;
    assert_eq!(rig.frame(&hud.scene), 1, "A again, rebuilt");
    let other = super::hud();
    assert_eq!(rig.frame(&other.scene), 2, "another World: every canvas");
}
