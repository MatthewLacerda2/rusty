//! A reflection probe's card open in the scene inspector is its selection:
//! the Scene view draws that probe's volume alone (`Scene::inspected_reflection_probe`),
//! and the cards follow it, one open at a time.

use egui::{Event, Modifiers, PointerButton, Pos2};
use glam::Vec3;

use super::draw_probes;
use rusty_scene::Scene;

/// One inspector frame of the probe cards with `events`, a second after the last
/// (past a card's open or close animation); returns each widget's label and centre.
fn frame(ctx: &egui::Context, scene: &mut Scene, events: Vec<Event>) -> Vec<(String, Pos2)> {
    let input = egui::RawInput {
        events,
        time: Some(ctx.input(|i| i.time) + 1.0),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
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

/// Click the widget whose label ends with `label`, then draw a settled frame.
fn click(ctx: &egui::Context, scene: &mut Scene, label: &str) {
    let drawn = frame(ctx, scene, Vec::new());
    let at = drawn
        .iter()
        .find(|(l, _)| l.ends_with(label))
        .expect("drawn")
        .1;
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

/// The open cards' Remove buttons, as a settled frame draws them.
fn open_cards(ctx: &egui::Context, scene: &mut Scene) -> usize {
    let drawn = frame(ctx, scene, Vec::new());
    drawn.iter().filter(|(l, _)| l.ends_with("Remove")).count()
}

fn three() -> (egui::Context, Scene) {
    let mut scene = Scene::new();
    for x in [0.0, 5.0, 10.0] {
        scene.reflection_probes.add(Vec3::X * x, Vec3::ONE);
    }
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    frame(&ctx, &mut scene, Vec::new());
    (ctx, scene)
}

#[test]
fn the_open_card_names_the_inspected_probe_and_one_is_open_at_a_time() {
    let (ctx, mut scene) = three();
    assert_eq!(scene.inspected_reflection_probe, None, "every card closed");
    click(&ctx, &mut scene, "Probe 1");
    assert_eq!(scene.inspected_reflection_probe, Some(1), "card 1 open");
    click(&ctx, &mut scene, "Probe 2");
    assert_eq!(scene.inspected_reflection_probe, Some(2), "card 2 takes it");
    assert_eq!(open_cards(&ctx, &mut scene), 1, "card 1 closed");
    click(&ctx, &mut scene, "Probe 2");
    assert_eq!(
        scene.inspected_reflection_probe, None,
        "card 2 closed again"
    );
    assert_eq!(open_cards(&ctx, &mut scene), 0);
}

/// `Editor.InspectReflectionProbe`'s write opens the card, as a click does.
#[test]
fn inspecting_a_probe_opens_its_card() {
    let (ctx, mut scene) = three();
    scene.inspect_reflection_probe(2).unwrap();
    assert_eq!(open_cards(&ctx, &mut scene), 1);
    assert_eq!(
        scene.inspected_reflection_probe,
        Some(2),
        "drawing keeps it"
    );
    scene.inspected_reflection_probe = None;
    assert_eq!(open_cards(&ctx, &mut scene), 0, "cleared, it closes");
}

#[test]
fn removing_the_inspected_probe_opens_no_other_card() {
    let (ctx, mut scene) = three();
    click(&ctx, &mut scene, "Probe 1");
    click(&ctx, &mut scene, "Remove");
    assert_eq!(scene.reflection_probes.len(), 2);
    assert_eq!(scene.inspected_reflection_probe, None);
    assert_eq!(
        open_cards(&ctx, &mut scene),
        0,
        "probe 2, now 1, stays closed"
    );
}
