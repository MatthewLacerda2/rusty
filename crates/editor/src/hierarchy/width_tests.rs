//! The Hierarchy panel is as wide as the user left it, never as its rows: a long
//! name scrolling into view is clipped inside the panel, which keeps its width
//! (and so does the Scene view beside it).

use super::DEFAULT_WIDTH;
use crate::EditorUi;
use rusty_scene::Scene;

const LONG: &str = "LongName_that_is_far_wider_than_any_hierarchy_panel_could_be_dragged_to_be";
const ROWS: usize = 120;
const LONG_ROW: usize = 60;

fn scene(long: bool) -> Scene {
    let mut scene = Scene::new();
    for i in 0..ROWS {
        let name = if long && i == LONG_ROW {
            LONG.into()
        } else {
            format!("Row_{i}")
        };
        scene.add_entity(name);
    }
    scene
}

/// What one frame showed: the panel's right edge, and the long name's painted
/// text as (its painted rect, its clip rect, whether it was elided).
struct Frame {
    right: f32,
    long: Option<(egui::Rect, egui::Rect, bool)>,
}

fn frame(ctx: &egui::Context, editor: &mut EditorUi, scene: &mut Scene, wheel: f32) -> Frame {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 600.0));
    let mut events = vec![egui::Event::PointerMoved(egui::pos2(60.0, 300.0))];
    if wheel != 0.0 {
        let phase = |phase| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, wheel),
            phase,
            modifiers: egui::Modifiers::NONE,
        };
        events.extend([
            phase(egui::TouchPhase::Start),
            phase(egui::TouchPhase::Move),
        ]);
    }
    let input = egui::RawInput {
        screen_rect: Some(screen),
        events,
        ..Default::default()
    };
    let mut right = 0.0;
    let mut out = ctx.run_ui(input, |ui| {
        super::draw(editor, ui, scene);
        right = ui.cursor().min.x;
    });
    out.textures_delta.clear();
    let long = out.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Text(text) if text.galley.job.text == LONG => Some((
            text.visual_bounding_rect(),
            clipped.clip_rect,
            text.galley.elided,
        )),
        _ => None,
    });
    Frame { right, long }
}

#[test]
fn a_long_name_scrolling_through_the_view_never_moves_the_panel_edge() {
    let ctx = egui::Context::default();
    let mut editor = EditorUi::new(Default::default());
    let mut long = scene(true);
    let top = [0.0, 0.0].map(|w| frame(&ctx, &mut editor, &mut long, w));
    assert!(top[1].long.is_none(), "the long row starts below the view");
    let right = top[1].right;
    assert!(
        (right - DEFAULT_WIDTH).abs() < 1.0,
        "the default width: {right}"
    );

    // Scroll down until the long row is in view, then back to the top.
    let shown = (0..40)
        .map(|_| frame(&ctx, &mut editor, &mut long, -40.0))
        .find(|f| f.long.is_some())
        .expect("the long row scrolls into view");
    let (painted, clip, elided) = shown.long.unwrap();
    assert!(elided, "the long name is truncated");
    assert!(
        painted.max.x <= right && clip.max.x <= right,
        "{painted:?} {clip:?} past {right}"
    );
    let back: Vec<Frame> = (0..40)
        .map(|_| frame(&ctx, &mut editor, &mut long, 40.0))
        .collect();
    assert!(
        back.last().unwrap().long.is_none(),
        "the long row left the view"
    );
    for f in top.iter().chain([&shown]).chain(&back) {
        assert_eq!(f.right, right, "the panel's edge moved");
    }

    let ctx = egui::Context::default();
    let mut editor = EditorUi::new(Default::default());
    let short = [0.0, 0.0].map(|w| frame(&ctx, &mut editor, &mut scene(false), w));
    assert_eq!(
        short[1].right, right,
        "the long name changed the panel's width"
    );
}
