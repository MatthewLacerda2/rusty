//! The unsaved mark the queue reads, which only a save clears.

use super::{ask, editor, frames, states};
use rusty_dev::bake::queue::{Runner, State};
use rusty_scene::lighting::bake_status::BakeStatus;

/// Loading would change it: the bake asks for a full save (`NeedsSceneSave`).
fn hand_edit(file: &std::path::Path) {
    let text = std::fs::read_to_string(file).unwrap();
    std::fs::write(file, text.replacen('{', "{\n  \"retired_field\": 1,", 1)).unwrap();
}

fn run(root: &std::path::Path, editor: &mut crate::EditorUi, scene: &mut rusty_scene::Scene) {
    ask(root, scene, &["a.scene"]);
    frames(editor, scene, |e| e.dev.bake_queue.running.is_some());
    frames(editor, scene, |e| e.dev.bake_queue.running.is_none());
}

/// Baked once first, so what the second owes is outside the document.
#[test]
fn gpu_a_bake_only_a_full_save_keeps_is_saved_before_it_reads_done() {
    let (root, mut editor, mut scene) = editor("editor_queue_needs_save");
    run(&root, &mut editor, &mut scene);
    let a = root.join("a.scene");
    hand_edit(&a);
    run(&root, &mut editor, &mut scene);
    assert_eq!(states(&root), [State::Done]);
    let text = std::fs::read_to_string(&a).unwrap();
    assert!(
        !text.contains("retired_field"),
        "the bake was recorded done unwritten"
    );
}

#[test]
fn gpu_a_bake_whose_full_save_fails_reads_failed_and_stops_the_queue() {
    let (root, mut editor, mut scene) = editor("editor_queue_save_fails");
    hand_edit(&root.join("a.scene"));
    // A folder where the full save writes the SH sidecar.
    std::fs::create_dir_all(root.join("a.scene.lighting.json")).unwrap();
    ask(&root, &mut scene, &["a.scene", "b.scene"]);
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_some()
    });
    frames(&mut editor, &mut scene, |e| {
        e.dev.bake_queue.running.is_none()
    });
    assert_eq!(states(&root), [State::Failed, State::Pending]);
    let rusty_dev::bake::queue::Read::Found(q) = rusty_dev::bake::queue::read(&root) else {
        panic!("no queue file")
    };
    let why = q.scenes[0].reason.clone().unwrap_or_default();
    assert!(why.contains("could not be saved"), "{why}");
    assert!(editor.is_dirty, "the lighting left unwritten reads unsaved");
    assert!(editor.current_scene_path().unwrap().ends_with("a.scene"));
}

/// A single bake's mark owing a full save survives the card's look at a queue on disk.
#[test]
fn drawing_the_card_leaves_a_bakes_unsaved_mark_alone() {
    let (root, mut editor, mut scene) = editor("editor_queue_card_reads");
    drop(Runner::start(&root, vec!["a.scene".into()]).unwrap());
    scene.mark_unsaved();
    editor.follow_edits(&mut scene);
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(Default::default(), |ui| {
        super::super::draw(ui, &mut editor, &mut scene);
    });
    out.textures_delta.clear();
    assert!(editor
        .dev
        .bake_queue
        .seen
        .as_ref()
        .is_some_and(|(_, l)| l.is_some()));
    assert!(
        editor.unsaved(&mut scene),
        "the card's draw cleared the mark"
    );
    assert!(!matches!(
        scene.lighting_bake_status,
        BakeStatus::Refused(_)
    ));
}
