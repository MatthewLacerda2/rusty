//! A bake refuses Play (#2042): `Editor.EnterPlay()` errors with the Play
//! button's reason while a `GenerateAsync` waits or its job runs, and is taken
//! once the bake ended, cancelled or not. Whichever was asked for first wins.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::async_tests::{frame, open_editor_in, run_out, status};
use super::{poll, take_request, Step};
use crate::EditorUi;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// One editor frame's bake drive; whether it asked for a repaint at once (not the
/// progress bar's timed one). The header read Play's refusal before it, so the
/// frame a bake stops must ask, or the window sleeps on a greyed-out Play.
fn frame_repaints_now(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) -> bool {
    let ctx = egui::Context::default();
    let now = Arc::new(AtomicBool::new(false));
    let seen = now.clone();
    ctx.set_request_repaint_callback(move |info| {
        seen.fetch_or(info.delay.is_zero(), Ordering::Relaxed);
    });
    let nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    poll(editor, &ctx, scene, console, &nav);
    now.load(Ordering::Relaxed)
}

/// `GenerateAsync(); EnterPlay()` in one line: the bake was asked for first, so
/// Play errors, nothing heads to Play, and the window's next frame starts the bake.
#[test]
fn gpu_play_asked_after_a_request_in_the_same_line_is_refused() {
    let (s, mut editor) = open_editor_in("play_after_request");
    let err = s
        .eval("Lighting.GenerateAsync(); Editor.EnterPlay()")
        .unwrap_err();
    assert!(err.contains("Editor.EnterPlay: Baking lighting"), "{err}");
    let app = &s.world().resources.application;
    assert!(!app.borrow().play_heading(), "Play was not asked for");
    assert!(status(&s).starts_with("requested"), "{}", status(&s));
    let mut console = ConsoleLogs::new();
    let scene = s.world().scene().clone();
    frame(&mut editor, &mut scene.borrow_mut(), &mut console);
    assert!(editor.dev.lightmap_bake.is_some(), "{:?}", console.messages);
    run_out(&mut editor, &scene, &mut console);
}

/// Refused while the job runs, with the same line the Play button's tooltip
/// shows; taken once it was cancelled.
///
/// The job is started by the frame's first half alone (`take_request`), never a
/// whole frame: a frame also advances the job, and an empty scene's lightmaps can
/// finish inside it on a fast GPU, past the step a cancel applies to (#2105). So
/// the cancel always reaches the lightmap step, and that step reads a cancel taken
/// before its poll as cancelled even when its worker already finished
/// (`LightmapBakeJob::poll`), whatever the GPU's speed.
#[test]
fn gpu_play_is_refused_while_the_job_runs_and_taken_after_a_cancel() {
    let (s, mut editor) = open_editor_in("play_during_bake");
    s.eval("Lighting.GenerateAsync()").unwrap();
    let mut console = ConsoleLogs::new();
    let scene = s.world().scene().clone();
    take_request(&mut editor, &mut scene.borrow_mut(), &mut console);
    let why = scene.borrow().play_refusal().expect("a bake runs");
    assert!(why.starts_with("Baking lighting"), "{why}");
    let err = s.eval("Editor.EnterPlay()").unwrap_err();
    assert!(err.contains(&format!("Editor.EnterPlay: {why}")), "{err}");
    match editor.dev.lightmap_bake.as_ref().map(|a| &a.step) {
        Some(Step::Lightmaps(job)) => job.cancel(),
        _ => panic!("the job was started, not advanced: {:?}", console.messages),
    }
    // The frame the cancel lands repaints at once, so the next frame draws Play
    // enabled without input.
    for _ in 0..4000 {
        let now = frame_repaints_now(&mut editor, &mut scene.borrow_mut(), &mut console);
        if editor.dev.lightmap_bake.is_none() {
            assert!(now, "the frame the bake ended asks for a repaint");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(editor.dev.lightmap_bake.is_none(), "{:?}", console.messages);
    assert!(status(&s).starts_with("cancelled"), "{}", status(&s));
    assert_eq!(scene.borrow().play_refusal(), None);
    assert_eq!(s.eval("Editor.EnterPlay()").unwrap(), "true");
}

/// The verb reads the scene's job state alone, so a faked running bake refuses it
/// without a GPU, and an ended one lets it through.
#[test]
fn enter_play_follows_the_job_state() {
    let s = rusty_dev::session::Session::new("").expect("an empty session boots");
    let scene = s.world().scene().clone();
    scene.borrow_mut().lighting_bake_running = true;
    let err = s.eval("Editor.EnterPlay()").unwrap_err();
    assert!(err.contains("Editor.EnterPlay: Baking lighting"), "{err}");
    scene.borrow_mut().lighting_bake_running = false;
    assert_eq!(s.eval("Editor.EnterPlay()").unwrap(), "true");
}

/// A request that never starts (no scene file to bake into) frees Play in the frame
/// it is refused, and that frame repaints at once; an idle frame does not.
#[test]
fn a_request_that_never_starts_repaints_at_once() {
    let mut editor = EditorUi::new(Default::default());
    let mut scene = Scene::new();
    let mut console = ConsoleLogs::new();
    assert!(
        !frame_repaints_now(&mut editor, &mut scene, &mut console),
        "idle"
    );
    scene.lighting_bake_requested = true;
    assert!(
        scene.play_refusal().is_some(),
        "the header saw Play refused"
    );
    assert!(frame_repaints_now(&mut editor, &mut scene, &mut console));
    assert_eq!(scene.play_refusal(), None);
    assert_eq!(scene.lighting_bake_status.state(), "refused");
}
