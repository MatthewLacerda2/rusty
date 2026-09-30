use glam::{Vec2, Vec4};

use super::*;
use crate::components::{
    CanvasComponent, CanvasGroupComponent, ImageComponent, RectMaskComponent,
    RectTransformComponent,
};
use crate::scene::Scene;

const SCREEN: Vec2 = Vec2::new(1920.0, 1080.0);

/// A canvas at scale 1 on [`SCREEN`].
fn canvas(scene: &mut Scene) -> u32 {
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    root
}

/// A `size`-sized element at `pos` (bottom-left anchored) under `parent`.
fn element(scene: &mut Scene, parent: u32, pos: Vec2, size: Vec2, texture: Option<&str>) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: pos,
        size_delta: size,
    };
    scene.world.set_rect_transform(id, Some(rt));
    let image = ImageComponent {
        texture: texture.map(str::to_string),
        ..Default::default()
    };
    scene.world.set_image(id, Some(image));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

fn build(scene: &Scene) -> Vec<CanvasMesh> {
    let layout = UiLayout::compute(&scene.world, SCREEN);
    let mut atlases = FontAtlases::default();
    build_canvas_meshes(
        &scene.world,
        &layout,
        SCREEN,
        &|_| Some(Vec2::splat(8.0)),
        &mut atlases,
    )
}

#[test]
fn solid_images_share_one_batch_and_textures_break_it() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene);
    for i in 0..3 {
        element(
            &mut scene,
            root,
            Vec2::splat(i as f32 * 10.0),
            Vec2::ONE,
            None,
        );
    }
    let meshes = build(&scene);
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].batches.len(), 1, "three solid bars, one draw");
    assert_eq!(meshes[0].vertices.len(), 18);

    element(&mut scene, root, Vec2::ZERO, Vec2::ONE, Some("a.png"));
    element(&mut scene, root, Vec2::ZERO, Vec2::ONE, Some("a.png"));
    element(&mut scene, root, Vec2::ZERO, Vec2::ONE, None);
    let batches = &build(&scene)[0].batches;
    let sources: Vec<_> = batches.iter().map(|b| b.source.clone()).collect();
    let png = UiSource::Texture("a.png".into());
    assert_eq!(sources, [UiSource::Solid, png, UiSource::Solid]);
}

#[test]
fn a_mask_breaks_the_batch_and_clips_its_subtree() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene);
    element(&mut scene, root, Vec2::ZERO, Vec2::ONE, None);
    let panel = element(
        &mut scene,
        root,
        Vec2::new(100.0, 200.0),
        Vec2::new(50.0, 40.0),
        None,
    );
    scene
        .world
        .set_rect_mask(panel, Some(RectMaskComponent::default()));
    element(&mut scene, panel, Vec2::ZERO, Vec2::splat(500.0), None);
    let batches = &build(&scene)[0].batches;
    assert_eq!(batches.len(), 2);
    let clip = batches[1].clip.expect("masked");
    // Top-left origin: y = 1080 - (200 + 40).
    assert_eq!((clip.x, clip.y, clip.w, clip.h), (100, 840, 50, 40));
    assert_eq!(batches[1].range.len(), 12, "the panel and its child");
}

#[test]
fn groups_multiply_alpha_and_inactive_subtrees_vanish() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene);
    let group = element(&mut scene, root, Vec2::ZERO, Vec2::ONE, None);
    let g = CanvasGroupComponent {
        alpha: 0.5,
        ..Default::default()
    };
    scene.world.set_canvas_group(group, Some(g.clone()));
    let child = element(&mut scene, group, Vec2::ZERO, Vec2::ONE, None);
    scene.world.set_canvas_group(child, Some(g));
    let mesh = &build(&scene)[0];
    assert_eq!(mesh.vertices[0].color, [1.0, 1.0, 1.0, 0.5]);
    assert_eq!(mesh.vertices[6].color, [1.0, 1.0, 1.0, 0.25]);

    scene.world.set_active(group, false);
    assert!(
        build(&scene).is_empty(),
        "an inactive parent hides its subtree"
    );
}

#[test]
fn vertices_land_in_ndc_and_canvases_keep_sort_order() {
    let mut scene = Scene::new();
    let front = canvas(&mut scene);
    scene.world.canvas_mut(front).expect("canvas").sort_order = 5;
    element(&mut scene, front, Vec2::ZERO, Vec2::ONE, None);
    let back = canvas(&mut scene);
    let e = element(&mut scene, back, Vec2::ZERO, SCREEN, None);
    scene.world.image_mut(e).expect("image").color = Vec4::new(1.0, 0.0, 0.0, 1.0);
    let meshes = build(&scene);
    assert_eq!(
        meshes.iter().map(|m| m.canvas).collect::<Vec<_>>(),
        [back, front]
    );
    let full = &meshes[0].vertices;
    assert_eq!(full[0].pos, [-1.0, -1.0]);
    assert_eq!(full[2].pos, [1.0, 1.0]);
    assert_eq!(full[0].uv, [0.0, 1.0], "v flips for the GPU");
}
