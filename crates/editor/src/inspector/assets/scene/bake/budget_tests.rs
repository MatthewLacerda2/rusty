//! A frame the window cannot draw takes probe steps for a time budget, not one
//! (`poll_undrawn`): nothing is painted there, and its frames come at the pacer's
//! retry, far apart.

use super::poll::paced;
use super::{start, Step};
use crate::EditorUi;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;
use std::time::Duration;

/// Runs the bake with `budget` a frame; returns how many frames ended inside the
/// probe stage and the console's lines.
fn frames_in_probes(tag: &str, budget: Option<Duration>) -> (usize, Vec<String>) {
    let path = rusty_test_support::temp::dir().join(format!("bake_budget_{tag}.scene"));
    let path = path.to_string_lossy().into_owned();
    let mut editor = EditorUi::new(Default::default());
    editor.set_current_scene_path(Some(path.clone()));
    let mut scene = Scene::new();
    scene.lighting_settings.reflections.resolution = 16;
    for x in [0.0, 4.0, 8.0] {
        let at = glam::Vec3::new(x, 1.0, 0.0);
        let i = scene.reflection_probes.add(at, glam::Vec3::splat(2.0));
        scene.reflection_probes.edit(i, |p| p.manual = true);
    }
    rusty_edit::save_to_file(&scene, &path).unwrap();
    let mut console = ConsoleLogs::new();
    start(&mut editor, &mut scene, &mut console);
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    let ctx = egui::Context::default();
    let mut frames = 0;
    for _ in 0..2000 {
        paced(&mut editor, &ctx, &mut scene, &mut console, &nav, budget);
        let Some(active) = &editor.dev.lightmap_bake else {
            break;
        };
        frames += usize::from(matches!(active.step, Step::Probes { .. }));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(editor.dev.lightmap_bake.is_none(), "{:?}", console.messages);
    (
        frames,
        console.messages.into_iter().map(|(l, _)| l).collect(),
    )
}

/// A budget no step outlasts takes the whole probe stage in the frame that enters
/// it, where one step a frame needs a frame per probe.
#[test]
fn gpu_an_undrawn_frame_takes_probe_steps_for_its_budget() {
    let (stepped, said) = frames_in_probes("one", None);
    if !said
        .iter()
        .any(|l| l.contains("Baking reflection probe 3 of 3"))
    {
        return; // no adapter: the probe step skipped
    }
    assert!(stepped >= 3, "a frame per probe, saw {stepped}");
    let (budgeted, said) = frames_in_probes("budget", Some(Duration::from_secs(600)));
    assert_eq!(budgeted, 0, "the stage ended in one frame: {said:?}");
    assert!(
        said.iter().any(|l| l.contains("Generated lighting")),
        "{said:?}"
    );
}
