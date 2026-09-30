//! Tests for Selectable transitions (#420): a ColorTint fade over unscaled time, an
//! instant first state, SpriteSwap, a separate target graphic and Disabled.

use glam::{Vec2, Vec4};

use super::fixture::{button, panel, scene, Handlers, Rig};
use crate::components::{SelectableTransition, SelectionState, TextComponent};

fn tint(rig: &Rig, id: u32) -> Vec4 {
    rig.scene.world.image(id).expect("image").state_tint
}

fn near(a: Vec4, b: Vec4) -> bool {
    (a - b).abs().max_element() < 1e-4
}

#[test]
fn color_tint_starts_at_its_state_and_fades_linearly_on_hover() {
    let (mut s, canvas) = scene();
    let btn = button(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let mut sel = s.world.selectable(btn).expect("sel").clone();
    sel.colors[0] = Vec4::new(1.0, 0.0, 0.0, 1.0);
    sel.colors[1] = Vec4::new(0.0, 0.0, 1.0, 1.0);
    sel.fade_duration = 0.2;
    s.world.set_selectable(btn, Some(sel));
    let mut rig = Rig::new(s, Handlers::default());
    rig.point(1000.0, 1000.0);
    rig.tick();
    rig.events.apply_transitions(&mut rig.scene.world, 0.1);
    assert!(
        near(tint(&rig, btn), Vec4::new(1.0, 0.0, 0.0, 1.0)),
        "no fade in"
    );
    rig.point(50.0, 50.0);
    rig.tick();
    rig.events.apply_transitions(&mut rig.scene.world, 0.1);
    assert!(
        near(tint(&rig, btn), Vec4::new(0.5, 0.0, 0.5, 1.0)),
        "half-way"
    );
    rig.events.apply_transitions(&mut rig.scene.world, 0.1);
    assert!(
        near(tint(&rig, btn), Vec4::new(0.0, 0.0, 1.0, 1.0)),
        "arrived"
    );
}

#[test]
fn sprite_swap_and_a_separate_target_graphic() {
    let (mut s, canvas) = scene();
    let btn = button(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let icon = panel(&mut s, btn, Vec2::ZERO, Vec2::splat(10.0));
    s.world.set_text(icon, Some(TextComponent::default()));
    let mut sel = s.world.selectable(btn).expect("sel").clone();
    sel.transition = SelectableTransition::SpriteSwap;
    sel.sprites[4] = Some("off.png".to_string());
    sel.target_graphic = Some(icon);
    sel.interactable = false;
    s.world.set_selectable(btn, Some(sel));
    let mut rig = Rig::new(s, Handlers::default());
    rig.tick();
    let world = &rig.scene.world;
    assert_eq!(rig.events.state_of(world, btn), SelectionState::Disabled);
    rig.events.apply_transitions(&mut rig.scene.world, 0.0);
    let image = rig.scene.world.image(icon).expect("image").clone();
    assert_eq!(image.shown_texture(), Some("off.png"));
    assert_eq!(
        rig.scene.world.image(btn).expect("image").override_texture,
        None
    );
    // Switching to ColorTint tints the target's Image and Text, instantly at 0 fade.
    let mut sel = rig.scene.world.selectable(btn).expect("sel").clone();
    sel.transition = SelectableTransition::ColorTint;
    sel.fade_duration = 0.0;
    rig.scene.world.set_selectable(btn, Some(sel.clone()));
    rig.events.apply_transitions(&mut rig.scene.world, 0.0);
    let disabled = sel.color(SelectionState::Disabled);
    assert!(near(tint(&rig, icon), disabled));
    assert_eq!(
        rig.scene.world.image(icon).expect("image").override_texture,
        None
    );
    let text_tint = rig.scene.world.text(icon).expect("text").state_tint;
    assert!(near(text_tint, disabled));
}
