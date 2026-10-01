//! Shape meshing (#425): the effect quads, batching with solid images and the
//! blend-mode batch break.

use glam::{Vec2, Vec4};

use crate::components::{
    CanvasComponent, ImageComponent, RectTransformComponent, ShapeComponent, ShapeGlow,
    ShapeShadow, UiBlend,
};
use crate::render::ui::mesh::{build_canvas_meshes, CanvasMesh, UiSource};
use crate::render::ui::text::atlas::FontAtlases;
use crate::render::ui::vertex::MODE_SHAPE;
use crate::scene::Scene;
use crate::ui::UiLayout;

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

/// A canvas holding one 100×50 element per entry, side by side.
fn hud(entries: &[(Option<ShapeComponent>, Option<ImageComponent>)]) -> Vec<CanvasMesh> {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    for (i, (shape, image)) in entries.iter().enumerate() {
        let id = scene.add_entity("E".to_string());
        let rt = RectTransformComponent {
            anchor_min: Vec2::ZERO,
            anchor_max: Vec2::ZERO,
            pivot: Vec2::ZERO,
            anchored_position: Vec2::new(i as f32 * 120.0, 0.0),
            size_delta: Vec2::new(100.0, 50.0),
            world_anchor: None,
        };
        scene.world.set_rect_transform(id, Some(rt));
        scene.world.set_shape(id, shape.clone());
        scene.world.set_image(id, image.clone());
        scene.set_parent(id, Some(root)).expect("parent exists");
    }
    let layout = UiLayout::compute(&scene.world, SCREEN);
    let mut atlases = FontAtlases::default();
    build_canvas_meshes(&scene.world, &layout, SCREEN, &|_| None, &mut atlases)
}

fn shape(blend: UiBlend) -> Option<ShapeComponent> {
    Some(ShapeComponent {
        blend,
        ..Default::default()
    })
}

#[test]
fn a_shape_is_one_quad_and_its_shadow_and_glow_draw_first() {
    let plain = hud(&[(shape(UiBlend::Normal), None)]);
    assert_eq!(plain[0].vertices.len(), 6);
    let v = plain[0].vertices[0];
    assert_eq!(v.sdf[0], MODE_SHAPE);
    assert_eq!(&v.local[2..], &[50.0, 25.0], "half size rides along");

    let lit = ShapeComponent {
        shadow: ShapeShadow {
            color: Vec4::new(0.0, 0.0, 0.0, 0.5),
            ..Default::default()
        },
        glow: ShapeGlow {
            size: 8.0,
            intensity: 2.0,
            color: Vec4::new(0.0, 1.0, 1.0, 1.0),
        },
        ..Default::default()
    };
    let m = &hud(&[(Some(lit), None)])[0];
    assert_eq!(m.vertices.len(), 18, "shadow + glow + body");
    assert_eq!(m.batches.len(), 1, "one draw");
    let (shadow, glow, body) = (m.vertices[0], m.vertices[6], m.vertices[12]);
    assert!(shadow.sdf[2] > 0.0, "the shadow is soft");
    assert_eq!(glow.sdf[3], 8.0, "the glow reaches 8 units");
    assert_eq!(glow.color, [0.0, 2.0, 2.0, 1.0], "intensity brightens");
    assert_eq!((body.sdf[2], body.sdf[3]), (0.0, 0.0));
    assert!(glow.local[0] < body.local[0], "the glow quad is larger");
}

#[test]
fn shapes_batch_with_solid_images_until_the_blend_changes() {
    let image = Some(ImageComponent::default());
    let same = hud(&[
        (shape(UiBlend::Normal), None),
        (None, image.clone()),
        (shape(UiBlend::Normal), None),
    ]);
    assert_eq!(same[0].batches.len(), 1);
    assert_eq!(same[0].batches[0].source, UiSource::Solid);

    // A mixed HUD: normal, additive neon, normal again — three draws.
    let mixed = hud(&[
        (shape(UiBlend::Normal), None),
        (shape(UiBlend::Additive), None),
        (shape(UiBlend::Additive), None),
        (shape(UiBlend::Normal), image),
    ]);
    let blends: Vec<UiBlend> = mixed[0].batches.iter().map(|b| b.blend).collect();
    assert_eq!(
        blends,
        vec![UiBlend::Normal, UiBlend::Additive, UiBlend::Normal]
    );
}
