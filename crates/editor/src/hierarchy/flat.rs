//! The Hierarchy's tree flattened into the rows it would show: one pre-order walk
//! of the expanded entities, so the panel can lay out only the rows in its scroll
//! view (`tree`). A folded branch's subtree is not walked at all.

use egui::collapsing_header::CollapsingState;
use rusty_scene::Scene;

/// One line of the tree, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub id: u32,
    /// How many branches it sits under: its indent in steps of `spacing.indent`.
    pub depth: usize,
    /// `None` for a leaf; for a branch, whether it is expanded.
    pub open: Option<bool>,
    /// The row of its parent branch, `None` for a root.
    pub parent: Option<usize>,
    /// One past its subtree's last row: an open branch's body is `index + 1..end`.
    pub end: usize,
}

pub(super) fn node_id(entity_id: u32) -> egui::Id {
    egui::Id::new(("hierarchy_node", entity_id))
}

pub(super) fn node_state(ctx: &egui::Context, scene: &Scene, entity_id: u32) -> CollapsingState {
    let id = node_id(entity_id);
    CollapsingState::load(ctx, id).unwrap_or_else(|| {
        let state = CollapsingState::load_with_default_open(ctx, id, !scene.is_bone(entity_id));
        state.store(ctx);
        state
    })
}

#[derive(Clone, Default)]
pub(super) struct Flattener {
    pub rows: Vec<Row>,
    kids: Vec<u32>,
}

impl Flattener {
    pub fn flatten(&mut self, ctx: &egui::Context, scene: &Scene) {
        self.rows.clear();
        self.kids.clear();
        for &id in scene.world.ids() {
            if scene.world.parent_id(id).is_none() {
                self.walk(ctx, scene, id, 0, None);
            }
        }
    }

    fn walk(
        &mut self,
        ctx: &egui::Context,
        scene: &Scene,
        id: u32,
        depth: usize,
        parent: Option<usize>,
    ) {
        if scene.world.name(id).is_none() || self.rows.len() >= scene.world.len() {
            return;
        }
        let at = self.kids.len();
        scene.world.push_children(id, &mut self.kids);
        let index = self.rows.len();
        let open = (self.kids.len() > at).then(|| node_state(ctx, scene, id).is_open());
        self.rows.push(Row {
            id,
            depth,
            open,
            parent,
            end: index + 1,
        });
        if open == Some(true) {
            for k in at..self.kids.len() {
                let child = self.kids[k];
                self.walk(ctx, scene, child, depth + 1, Some(index));
            }
        }
        self.kids.truncate(at);
        self.rows[index].end = self.rows.len();
    }
}
