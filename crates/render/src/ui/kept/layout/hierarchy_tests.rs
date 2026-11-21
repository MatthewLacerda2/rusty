use glam::Vec2;

use super::{element, ui, Rig};
use rusty_components::{CanvasComponent, CanvasRenderMode, WorldAnchor};

#[test]
fn reordering_siblings_lays_them_out_again() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    let group = ui.scene.world.parent_id(ui.sized).expect("in the group");
    element(&mut ui.scene, group);
    assert!(rig.frame(&ui.scene.world));
    ui.scene.set_parent(ui.sized, Some(group)).expect("parent");
    assert!(rig.frame(&ui.scene.world), "under a layout group");
    assert!(!rig.frame(&ui.scene.world));
    ui.scene.set_parent(group, Some(ui.canvas)).expect("parent");
    assert!(rig.frame(&ui.scene.world), "under a plain parent");
    assert!(!rig.frame(&ui.scene.world));
}

#[test]
fn a_nested_canvas_becoming_a_root_is_laid_out_and_back() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    let holder = ui.scene.add_entity("Holder".to_string());
    let deeper = ui.scene.add_entity("Deeper".to_string());
    ui.scene
        .set_parent(holder, Some(ui.canvas))
        .expect("parent");
    ui.scene.set_parent(deeper, Some(holder)).expect("parent");
    let inner = ui.scene.add_entity("Inner".to_string());
    let canvas = CanvasComponent::default();
    ui.scene.world.set_canvas(inner, Some(canvas));
    ui.scene.set_parent(inner, Some(deeper)).expect("parent");
    element(&mut ui.scene, inner);
    assert!(rig.frame(&ui.scene.world));
    ui.scene.set_parent(inner, None).expect("unparent");
    assert!(rig.frame(&ui.scene.world), "a root now: its HUD appears");
    ui.scene.set_parent(inner, Some(deeper)).expect("parent");
    assert!(rig.frame(&ui.scene.world), "nested again: its HUD goes");
}

#[test]
fn a_nested_canvas_s_own_fields_lay_nothing_out() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    let inner = element(&mut ui.scene, ui.canvas);
    ui.scene
        .world
        .set_canvas(inner, Some(CanvasComponent::default()));
    element(&mut ui.scene, inner);
    assert!(rig.frame(&ui.scene.world));
    let mut c = ui.scene.world.canvas_mut(inner).expect("canvas");
    c.reference_resolution = Vec2::new(1280.0, 720.0);
    (c.match_width_or_height, c.sort_order) = (1.0, 7);
    c.render_mode = CanvasRenderMode::WorldSpace;
    drop(c);
    assert!(!rig.frame(&ui.scene.world), "its root lays it out");
    ui.scene.set_parent(inner, None).expect("unparent");
    assert!(rig.frame(&ui.scene.world), "a root now: its fields count");
    ui.scene.world.canvas_mut(inner).expect("canvas").sort_order = 3;
    assert!(rig.frame(&ui.scene.world), "a root's sort order");
}

#[test]
fn an_anchor_no_marker_is_placed_from_never_follows_the_camera() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    let target = Some(ui.scene.add_entity("Crate".to_string()));
    let sign = ui.scene.add_entity("Sign".to_string());
    let world_canvas = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        ..Default::default()
    };
    ui.scene.world.set_canvas(sign, Some(world_canvas));
    let on_sign = element(&mut ui.scene, sign);
    let loose = element(&mut ui.scene, sign);
    ui.scene
        .set_parent(loose, None)
        .expect("outside any canvas");
    for id in [on_sign, loose] {
        let anchor = WorldAnchor {
            target,
            ..Default::default()
        };
        let mut rt = ui.scene.world.rect_transform_mut(id).expect("rt");
        rt.world_anchor = Some(anchor);
    }
    assert!(rig.frame(&ui.scene.world));
    rig.camera.as_mut().expect("camera").yaw += 5.0;
    assert!(!rig.frame(&ui.scene.world), "a turn moves no marker");
}
