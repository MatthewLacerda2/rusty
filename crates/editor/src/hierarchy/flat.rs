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

/// The id a branch's expand state is kept under in egui's memory: per entity, so
/// a fold survives the branch moving in the list, scrolling, or the panel closing.
pub(super) fn node_id(entity_id: u32) -> egui::Id {
    egui::Id::new(("hierarchy_node", entity_id))
}

/// A branch's expand state. A branch seen for the first time starts open unless
/// it is a skeleton bone (a skeleton is ~65 of them), and that default is stored
/// so later frames read it instead of walking the bone's ancestors again.
pub(super) fn node_state(ctx: &egui::Context, scene: &Scene, entity_id: u32) -> CollapsingState {
    let id = node_id(entity_id);
    CollapsingState::load(ctx, id).unwrap_or_else(|| {
        let state = CollapsingState::load_with_default_open(ctx, id, !scene.is_bone(entity_id));
        state.store(ctx);
        state
    })
}

/// Walks the scene into rows, reusing its buffers from frame to frame.
#[derive(Clone, Default)]
pub(super) struct Flattener {
    pub rows: Vec<Row>,
    /// Child ids of every branch on the walk's path, each branch's run pushed
    /// above its parent's: one buffer for the whole walk instead of a `Vec` per row.
    kids: Vec<u32>,
}

impl Flattener {
    /// Rebuild `rows` from the scene's roots (entities without a parent, in the
    /// World's order) and the expand state in `ctx`. An entity with no name is
    /// skipped with its subtree, as it has always been.
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
        // A well-formed tree has at most one row per entity; past that the
        // `children` lists hold a cycle, which ends the walk instead of the stack.
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
