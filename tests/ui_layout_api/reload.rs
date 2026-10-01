//! Both layout components survive a scene save → load, and the reloaded scene lays
//! out identically — the pass is a pure function of scene and screen size (#421).

use glam::{Vec2, Vec4};
use rusty::components::{
    CanvasComponent, LayoutAxisFit, LayoutElementComponent, LayoutGroupComponent, LayoutKind,
    RectTransformComponent,
};
use rusty::scene::Scene;
use rusty::ui::UiLayout;

/// A canvas holding a padded vertical list of three fitted rows.
fn menu() -> (Scene, Vec<u32>) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    let list = scene.add_entity("List".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(400.0, 0.0),
        ..RectTransformComponent::default()
    };
    scene.world.set_rect_transform(list, Some(rt.clone()));
    let group = LayoutGroupComponent {
        kind: LayoutKind::Vertical,
        padding: Vec4::splat(8.0),
        spacing: Vec2::new(0.0, 4.0),
        child_force_expand_height: false,
        ..LayoutGroupComponent::default()
    };
    scene.world.set_layout_group(list, Some(group));
    let fit = LayoutElementComponent {
        vertical_fit: LayoutAxisFit::PreferredSize,
        ..LayoutElementComponent::default()
    };
    scene.world.set_layout_element(list, Some(fit));
    scene.set_parent(list, Some(root)).unwrap();
    let mut ids = vec![list];
    for _ in 0..3 {
        let row = scene.add_entity("Row".to_string());
        scene.world.set_rect_transform(row, Some(rt.clone()));
        let le = LayoutElementComponent {
            preferred_height: Some(30.0),
            ..LayoutElementComponent::default()
        };
        scene.world.set_layout_element(row, Some(le));
        scene.set_parent(row, Some(list)).unwrap();
        ids.push(row);
    }
    (scene, ids)
}

#[test]
fn layout_components_round_trip_and_lay_out_identically_after_reload() {
    let (scene, ids) = menu();
    let screen = Vec2::new(1920.0, 1080.0);
    let before = UiLayout::compute(&scene.world, screen);
    // 8 + 3·30 + 2·4 + 8 tall, rows 384 wide inside the padding.
    let list = before.get(ids[0]).unwrap().rect;
    assert!((list.1 - Vec2::new(400.0, 114.0)).abs().max_element() < 1e-3);
    assert!((before.get(ids[3]).unwrap().rect.1.x - 384.0).abs() < 1e-3);
    let path = crate::temp::dir()
        .join(format!("rusty_ui_layout_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    for &id in &ids {
        let a = scene.world.layout_element(id).map(|e| e.clone());
        assert_eq!(loaded.world.layout_element(id).map(|e| e.clone()), a);
    }
    let group = scene.world.layout_group(ids[0]).map(|g| g.clone());
    assert_eq!(loaded.world.layout_group(ids[0]).map(|g| g.clone()), group);
    assert_eq!(UiLayout::compute(&loaded.world, screen), before);
}
