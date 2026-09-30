//! SDF text screenshot (#419): an outlined, glowing "I" rendered headless over
//! black, asserted by pixel properties along the scanline through its stem rather
//! than a byte-exact golden: black far away, then the cyan glow brightening
//! towards the glyph, then the red outline, then the white fill at the centre.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{
    CanvasComponent, ImageComponent, RectTransformComponent, TextAlignment, TextComponent,
};
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 256;

fn stretched(scene: &mut Scene, parent: u32) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ONE,
        size_delta: Vec2::ZERO,
        ..Default::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

fn neon_label() -> Scene {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    let backdrop = stretched(&mut scene, root);
    let black = ImageComponent {
        color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        ..Default::default()
    };
    scene.world.set_image(backdrop, Some(black));
    let label = stretched(&mut scene, root);
    let text = TextComponent {
        text: "I".into(),
        font_size: 200.0,
        color: Vec4::ONE,
        alignment: TextAlignment::MiddleCenter,
        outline_width: 0.04,
        outline_color: Vec4::new(1.0, 0.0, 0.0, 1.0),
        glow_size: 0.15,
        glow_color: Vec4::new(0.0, 1.0, 1.0, 1.0),
        ..Default::default()
    };
    scene.world.set_text(label, Some(text));
    scene
}

#[test]
fn outlined_glowing_text_layers_fill_outline_and_glow() {
    let path = std::env::temp_dir().join("rusty_ui_text.png");
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    if !capture(&neon_label(), &cam, &path, SIZE, SIZE).expect("capture must not error") {
        eprintln!("[ui] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let row: Vec<[u8; 3]> = (0..SIZE / 2)
        .map(|x| img.get_pixel(x, SIZE / 2).0)
        .collect();
    let white = |p: &[u8; 3]| p.iter().all(|&c| c > 240);
    let red = |p: &[u8; 3]| p[0] > 200 && p[1] < 60 && p[2] < 60;
    let glow = |p: &[u8; 3]| p[0] < 40 && p[1] > 40 && p[2] > 40 && p[1].abs_diff(p[2]) < 8;
    assert_eq!(
        row[4],
        [0, 0, 0],
        "far from the glyph is the black backdrop"
    );
    let centre = img.get_pixel(SIZE / 2, SIZE / 2).0;
    assert!(
        white(&centre),
        "the stem's centre is the white fill: {centre:?}"
    );
    let first = |f: &dyn Fn(&[u8; 3]) -> bool| row.iter().position(f);
    let (Some(g), Some(r), Some(w)) = (first(&glow), first(&red), first(&white)) else {
        panic!("glow, outline and fill all appear left of centre: {row:?}");
    };
    assert!(g < r && r < w, "glow {g} < outline {r} < fill {w}");
    let brighter_inwards = row[g..r].windows(2).all(|p| p[1][1] + 2 >= p[0][1]);
    assert!(
        brighter_inwards,
        "the glow brightens towards the glyph: {:?}",
        &row[g..r]
    );
    assert!(
        r - g >= 8,
        "the glow reaches well past the outline ({} px)",
        r - g
    );
}
