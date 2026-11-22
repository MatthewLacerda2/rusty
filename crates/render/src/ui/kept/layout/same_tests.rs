use glam::Vec3;

use super::{element, ui, Rig, Why};
use rusty_components::{LayoutGroupComponent, WorldAnchor};

#[test]
fn a_write_of_the_same_value_lays_nothing_out() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    rig.frame(&ui.scene.world);
    let group = ui.scene.world.parent_id(ui.sized).expect("in the group");
    let w = &mut ui.scene.world;
    drop(w.rect_transform_mut(ui.plain));
    drop(w.text_mut(ui.sized));
    drop(w.image_mut(ui.icon));
    drop(w.layout_element_mut(ui.icon));
    drop(w.layout_group_mut(group));
    drop(w.canvas_mut(ui.canvas));
    drop(w.transform_mut(ui.plain));
    w.set_active(ui.sized, true);
    w.set_layout_group(group, Some(LayoutGroupComponent::default()));
    assert!(!rig.frame(&ui.scene.world), "stamped, nothing moved");
    let w = &mut ui.scene.world;
    w.text_mut(ui.sized).expect("text").text = "AMMO 31".into();
    assert!(rig.frame(&ui.scene.world), "a sized string that did move");
    assert_eq!(rig.why(), Some(Why::Content));
}

#[test]
fn an_inactive_marker_reads_no_camera_until_it_is_activated() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    let foe = ui.scene.add_entity("Foe".to_string());
    let pool: Vec<u32> = (0..3)
        .map(|_| {
            let number = element(&mut ui.scene, ui.canvas);
            let w = &mut ui.scene.world;
            let anchor = WorldAnchor {
                target: Some(foe),
                ..Default::default()
            };
            w.rect_transform_mut(number).expect("rt").world_anchor = Some(anchor);
            w.set_active(number, false);
            number
        })
        .collect();
    assert!(rig.frame(&ui.scene.world));
    for i in 0..3 {
        rig.camera.as_mut().expect("camera").yaw += 2.0;
        let w = &mut ui.scene.world;
        w.transform_mut(foe).expect("t").position = Vec3::new(i as f32, 0.0, -4.0);
        assert!(!rig.frame(&ui.scene.world), "tick {i}: a pooled number");
    }
    ui.scene.world.set_active(pool[1], true);
    assert!(rig.frame(&ui.scene.world), "a number shown");
    assert_eq!(rig.why(), Some(Why::Structure));
    rig.camera.as_mut().expect("camera").yaw += 2.0;
    assert!(rig.frame(&ui.scene.world), "it follows the camera");
    assert_eq!(rig.why(), Some(Why::Marker));
    ui.scene.world.transform_mut(foe).expect("t").position.y = 1.0;
    assert!(rig.frame(&ui.scene.world), "and its target");
    assert_eq!(rig.why(), Some(Why::Marker));
    assert!(rig.frame(&super::ui().scene.world), "another World");
    assert_eq!(rig.why(), Some(Why::Rebuild));
}

#[test]
fn a_marker_hidden_behind_the_camera_then_activated_is_laid_out() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    let foe = ui.scene.add_entity("Foe".to_string());
    let w = &mut ui.scene.world;
    w.transform_mut(foe).expect("t").position = Vec3::new(0.0, 0.0, 10.0);
    let number = element(&mut ui.scene, ui.canvas);
    let w = &mut ui.scene.world;
    let anchor = WorldAnchor {
        target: Some(foe),
        hide_when_behind: true,
        ..Default::default()
    };
    w.rect_transform_mut(number).expect("rt").world_anchor = Some(anchor);
    w.set_active(number, false);
    assert!(rig.frame(&ui.scene.world), "the foe behind the camera");
    rig.camera.as_mut().expect("camera").yaw += 180.0;
    assert!(
        !rig.frame(&ui.scene.world),
        "an inactive number reads no camera"
    );
    ui.scene.world.set_active(number, true);
    assert!(rig.frame(&ui.scene.world), "the number shown, now in view");
    assert_eq!(rig.why(), Some(Why::Structure));
}

#[test]
fn an_equal_texture_table_is_kept_and_held_in_its_place() {
    let (ui, mut rig) = (ui(), Rig::new());
    rig.frame(&ui.scene.world);
    rig.textures = std::rc::Rc::new((*rig.textures).clone());
    assert!(!rig.frame(&ui.scene.world), "equal sizes, a new table");
    let held = rig.kept.held.as_ref().expect("held");
    assert!(
        std::rc::Rc::ptr_eq(&held.textures, &rig.textures),
        "swapped in"
    );
}
