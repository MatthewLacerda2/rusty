//! The Hierarchy's flattened tree: the rows a walk gives, and that a big scene
//! lays out only the rows in view.

use egui::collapsing_header::CollapsingState;

use super::flat::{node_id, Flattener, Row};
use super::tree::{Rows, SCROLL_SALT};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

fn fold(ctx: &egui::Context, entity_id: u32) {
    let mut state = CollapsingState::load_with_default_open(ctx, node_id(entity_id), true);
    state.set_open(false);
    state.store(ctx);
}

fn rows(ctx: &egui::Context, scene: &Scene) -> Vec<(u32, usize, Option<bool>)> {
    let mut flat = Flattener::default();
    flat.flatten(ctx, scene);
    flat.rows.iter().map(|r| (r.id, r.depth, r.open)).collect()
}

#[test]
fn the_walk_lists_expanded_rows_in_pre_order_and_skips_a_folded_branch() {
    let mut scene = Scene::new();
    let [a, b, c, d, e, f] = ["A", "B", "C", "D", "E", "F"].map(|n| scene.add_entity(n.into()));
    scene.set_parent(b, Some(a)).unwrap();
    scene.set_parent(c, Some(b)).unwrap();
    scene.set_parent(d, Some(a)).unwrap();
    scene.set_parent(f, Some(e)).unwrap();
    let ctx = egui::Context::default();
    let open = Some(true);
    let expanded = [
        (a, 0, open),
        (b, 1, open),
        (c, 2, None),
        (d, 1, None),
        (e, 0, open),
        (f, 1, None),
    ];
    assert_eq!(rows(&ctx, &scene), expanded);

    fold(&ctx, b);
    let folded = [
        (a, 0, open),
        (b, 1, Some(false)),
        (d, 1, None),
        (e, 0, open),
        (f, 1, None),
    ];
    assert_eq!(rows(&ctx, &scene), folded);
}

#[test]
fn each_row_knows_its_parent_row_and_where_its_subtree_ends() {
    let mut scene = Scene::new();
    let [a, b, c, d] = ["A", "B", "C", "D"].map(|n| scene.add_entity(n.into()));
    scene.set_parent(b, Some(a)).unwrap();
    scene.set_parent(c, Some(b)).unwrap();
    let mut flat = Flattener::default();
    flat.flatten(&egui::Context::default(), &scene);
    let row = |id, depth, open, parent, end| Row {
        id,
        depth,
        open,
        parent,
        end,
        group: None,
    };
    let want = [
        row(a, 0, Some(true), None, 3),
        row(b, 1, Some(true), Some(0), 3),
        row(c, 2, None, Some(1), 3),
        row(d, 0, None, None, 4),
    ];
    assert_eq!(flat.rows, want);
}

#[test]
fn a_bone_subtree_starts_folded() {
    let mut scene = Scene::new();
    let rig = create_entity(&mut scene, "Rig", Some(Primitive::Box));
    let [hip, knee] = ["Hip", "Knee"].map(|n| scene.add_entity(n.into()));
    scene.set_parent(hip, Some(rig)).unwrap();
    scene.set_parent(knee, Some(hip)).unwrap();
    scene.world.mesh_mut(rig).unwrap().skeleton.bones = vec![Some(hip), Some(knee)];
    let got = rows(&egui::Context::default(), &scene);
    assert_eq!(got, [(rig, 0, Some(true)), (hip, 1, Some(false))]);
}

#[test]
fn a_big_hierarchy_scrolled_to_its_middle_lays_out_only_the_rows_in_view() {
    // 1,000 branches with one child each: 2,000 rows, all expanded.
    let mut scene = Scene::new();
    let branches: Vec<u32> = (0..1000)
        .map(|i| {
            let branch = scene.add_entity(format!("Branch_{i}"));
            let leaf = scene.add_entity(format!("Leaf_{i}"));
            scene.set_parent(leaf, Some(branch)).unwrap();
            branch
        })
        .collect();
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 600.0));
    let mut pitch = 0.0;
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            pitch = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
            scroll_to(ui, 1000.0 * pitch);
            draw(ui, &scene);
        });
        out.textures_delta.clear();
    }
    // A branch's twisty is a widget under its node id: count the ones laid out.
    let drawn: Vec<usize> = (0..branches.len())
        .filter(|&i| ctx.read_response(node_id(branches[i])).is_some())
        .collect();
    let in_view = (screen.height() / pitch) as usize / 2; // every other row a branch
    assert!(
        !drawn.is_empty() && drawn.len() <= in_view + 2,
        "laid out {} branches, {in_view} in view",
        drawn.len()
    );
    assert!(drawn.contains(&500), "row 1,000 is in view: {drawn:?}");
}

fn scroll_to(ui: &egui::Ui, y: f32) {
    let id = ui.make_persistent_id(egui::IdSalt::new(SCROLL_SALT));
    let mut state = egui::scroll_area::State::load(ui.ctx(), id).unwrap_or_default();
    state.offset.y = y;
    state.store(ui.ctx(), id);
}

fn draw(ui: &mut egui::Ui, scene: &Scene) {
    let mut flat = Flattener::default();
    flat.flatten(ui.ctx(), scene);
    let (mut sel, mut asset, mut doomed, mut action) = (None, None, None, None);
    let marks = Default::default();
    let mut rows = Rows {
        sel_entity: &mut sel,
        sel_asset: &mut asset,
        delete: &mut doomed,
        theme: crate::theme::Theme::default(),
        marks: &marks,
        scene_action: &mut action,
    };
    rows.draw(ui, scene, &flat.rows);
}
