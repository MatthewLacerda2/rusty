//! A reflection probe's card open in the scene inspector is its selection (#2525):
//! the Scene view draws that probe's volume alone (`Scene::inspected_reflection_probe`).

use egui::{Event, Modifiers, PointerButton, Pos2};
use glam::Vec3;

use super::draw_probes;
use rusty_scene::Scene;

/// One inspector frame of the probe cards with `events`, a second after the last
/// (past a card's open or close animation), the field cleared as the editor's
/// frame clears it; returns each card header's label and centre.
fn frame(ctx: &egui::Context, scene: &mut Scene, events: Vec<Event>) -> Vec<(String, Pos2)> {
    let input = egui::RawInput {
        events,
        time: Some(ctx.input(|i| i.time) + 1.0),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        scene.inspected_reflection_probe = None;
        draw_probes(ui, scene);
    });
    output.textures_delta.clear();
    let tree = output
        .platform_output
        .accesskit_update
        .expect("accesskit on");
    (tree.nodes.iter())
        .filter_map(|(_, n)| {
            let (b, label) = (n.bounds()?, n.label()?);
            let centre = Pos2::new((b.x0 + b.x1) as f32 / 2.0, (b.y0 + b.y1) as f32 / 2.0);
            Some((label.to_string(), centre))
        })
        .collect()
}

/// Click the card header labelled `label`, then draw a settled frame.
fn open(ctx: &egui::Context, scene: &mut Scene, label: &str) {
    let drawn = frame(ctx, scene, Vec::new());
    let at = drawn.iter().find(|(l, _)| l == label).expect("drawn").1;
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(ctx, scene, vec![Event::PointerMoved(at)]);
    frame(ctx, scene, vec![press(true), press(false)]);
    frame(ctx, scene, Vec::new());
}

#[test]
fn the_open_card_names_the_inspected_probe_and_a_closed_one_none() {
    let mut scene = Scene::new();
    for x in [0.0, 5.0, 10.0] {
        scene.reflection_probes.add(Vec3::X * x, Vec3::ONE);
    }
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    frame(&ctx, &mut scene, Vec::new());
    assert_eq!(scene.inspected_reflection_probe, None, "every card closed");
    open(&ctx, &mut scene, "Probe 1");
    assert_eq!(scene.inspected_reflection_probe, Some(1), "card 1 open");
    open(&ctx, &mut scene, "Probe 2");
    assert_eq!(
        scene.inspected_reflection_probe,
        Some(1),
        "the first open card wins"
    );
    open(&ctx, &mut scene, "Probe 1");
    assert_eq!(
        scene.inspected_reflection_probe,
        Some(2),
        "card 1 closed again"
    );
}
