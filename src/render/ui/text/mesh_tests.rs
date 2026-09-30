//! Text in the UI mesh (#419): a label batches as one draw per font, beside images.

use glam::Vec2;

use crate::components::{CanvasComponent, ImageComponent, RectTransformComponent, TextComponent};
use crate::render::ui::mesh::{build_canvas_meshes, UiSource};
use crate::render::ui::text::atlas::FontAtlases;
use crate::scene::Scene;
use crate::ui::UiLayout;

const SCREEN: Vec2 = Vec2::new(1920.0, 1080.0);

#[test]
fn a_label_is_one_font_batch_after_its_panel() {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    let label = scene.add_entity("Label".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(600.0, 100.0),
        ..Default::default()
    };
    scene.world.set_rect_transform(label, Some(rt));
    scene
        .world
        .set_image(label, Some(ImageComponent::default()));
    let text = TextComponent {
        text: "AMMO 30".into(),
        ..Default::default()
    };
    scene.world.set_text(label, Some(text));
    scene.set_parent(label, Some(root)).expect("parent exists");

    let layout = UiLayout::compute(&scene.world, SCREEN);
    let mut atlases = FontAtlases::default();
    let meshes = build_canvas_meshes(&scene.world, &layout, SCREEN, &|_| None, &mut atlases);
    let batches = &meshes[0].batches;
    let sources: Vec<_> = batches.iter().map(|b| b.source.clone()).collect();
    assert_eq!(sources, [UiSource::Solid, UiSource::Font(None)]);
    assert_eq!(batches[1].range.len(), 6 * 6, "six glyphs, one draw");
    let text_vertex = meshes[0].vertices[batches[1].range.start as usize];
    assert_eq!(text_vertex.sdf[0], 1.0, "text samples its atlas as a field");
    assert_eq!(meshes[0].vertices[0].sdf[0], 0.0, "the panel is an image");
    assert!(atlases.atlases[&None].dirty, "new glyphs await upload");
}
