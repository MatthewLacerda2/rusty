use glam::{Vec2, Vec4};

use super::backdrop::blur_level;
use super::BatchUniform;
use crate::components::{
    BackdropFilterComponent, CanvasComponent, CanvasGroupComponent, ImageComponent, MaskComponent,
    RectTransformComponent,
};
use crate::render::ui::mesh::{build_canvas_meshes, CanvasMesh, UiClip};
use crate::render::ui::text::atlas::FontAtlases;
use crate::scene::Scene;
use crate::ui::UiLayout;

const SCREEN: Vec2 = Vec2::new(200.0, 100.0);

/// A `size` element at `pos` (bottom-left anchored) under `parent`, with an Image.
fn element(scene: &mut Scene, parent: u32, pos: Vec2, size: Vec2) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: pos,
        size_delta: size,
        world_anchor: None,
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.world.set_image(id, Some(ImageComponent::default()));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

/// A scene with a canvas at scale 1 on [`SCREEN`].
fn canvas() -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: SCREEN,
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    (scene, root)
}

fn build(scene: &Scene) -> CanvasMesh {
    let layout = UiLayout::compute(&scene.world, SCREEN);
    let mut atlases = FontAtlases::default();
    let tex = |_: &str| Some(Vec2::splat(8.0));
    let meshes = build_canvas_meshes(&scene.world, &layout, SCREEN, &tex, &mut atlases);
    meshes.into_iter().next().expect("one canvas")
}

#[test]
fn nested_masks_chain_and_hidden_graphics_only_shape_the_clip() {
    let (mut scene, root) = canvas();
    let outer = element(&mut scene, root, Vec2::ZERO, Vec2::splat(80.0));
    scene.world.set_mask(outer, Some(MaskComponent::default()));
    let inner = element(&mut scene, outer, Vec2::splat(10.0), Vec2::splat(40.0));
    let hidden = MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(inner, Some(hidden));
    scene.world.image_mut(inner).expect("image").color.w = 0.5;
    let leaf = element(&mut scene, inner, Vec2::ZERO, Vec2::splat(500.0));
    let _ = leaf;
    let mesh = build(&scene);
    // Two mask graphics recorded: the outer under no mask, the inner under it.
    let masks: Vec<_> = mesh.masks.iter().map(|m| (m.id, m.clip.mask)).collect();
    assert_eq!(masks, [(outer, None), (inner, Some(outer))]);
    // The inner graphic's coverage is its colour alpha.
    let first = mesh.masks[1].range.start as usize;
    assert_eq!(mesh.vertices[first].color[3], 0.5);
    // Drawn: the outer graphic (unmasked) and the leaf (inner mask, cut to its rect).
    let drawn: Vec<UiClip> = mesh.batches.iter().map(|b| b.clip).collect();
    assert_eq!(drawn.len(), 2, "the hidden inner graphic draws nothing");
    assert_eq!(drawn[0].mask, None);
    assert_eq!(drawn[1].mask, Some(inner));
    let r = drawn[1].rect.expect("cut to the inner mask's rect");
    assert_eq!((r.x, r.y, r.w, r.h), (10, 50, 40, 40));
}

#[test]
fn a_backdrop_batch_precedes_its_graphic_on_overlay_canvases_only() {
    let (mut scene, root) = canvas();
    let panel = element(&mut scene, root, Vec2::ZERO, Vec2::splat(50.0));
    scene.world.image_mut(panel).expect("image").color = Vec4::new(0.0, 0.0, 0.0, 0.0);
    let filter = BackdropFilterComponent {
        blur_radius: 12.0,
        ..Default::default()
    };
    scene.world.set_backdrop_filter(panel, Some(filter));
    let fade = CanvasGroupComponent {
        alpha: 0.5,
        ..Default::default()
    };
    scene.world.set_canvas_group(panel, Some(fade));
    let mesh = build(&scene);
    // The glass shows though the panel's own colour is fully transparent.
    assert_eq!(mesh.batches.len(), 1);
    let backdrop = mesh.batches[0].backdrop.expect("a backdrop batch");
    assert_eq!(backdrop.radius, 12.0);
    assert_eq!(
        mesh.vertices[0].color,
        [1.0, 1.0, 1.0, 0.5],
        "group alpha only"
    );

    let world = CanvasComponent {
        render_mode: crate::components::CanvasRenderMode::WorldSpace,
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(world));
    scene.world.image_mut(panel).expect("image").color.w = 1.0;
    let mesh = build(&scene);
    assert!(mesh.batches.iter().all(|b| b.backdrop.is_none()));
}

#[test]
fn blur_levels_double_their_reach() {
    assert_eq!(blur_level(0.0, 2), 0);
    assert_eq!(blur_level(4.0, 2), 0);
    assert_eq!(blur_level(8.0, 2), 1);
    assert_eq!(blur_level(16.0, 2), 2);
    assert_eq!(blur_level(16.0, 4), 1, "a coarser chain needs fewer levels");
    assert_eq!(blur_level(1e6, 2), super::backdrop::MAX_LEVEL);
}

#[test]
fn a_batch_slot_carries_clip_and_filter() {
    let slot = BatchUniform::new(&UiClip::default(), SCREEN, None);
    assert_eq!(
        slot.grade,
        [1.0, 1.0, 0.0, 0.0],
        "neutral without a backdrop"
    );
    assert_eq!(slot.clip, crate::render::ui::clip::NO_CLIP);
}
