use egui::collapsing_header::CollapsingState;
use rusty_scene::runtime::membership::Member;
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
    /// A header row: `id` is none.
    pub group: Option<Group>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Group {
    Scene(u32),
    DontDestroyOnLoad,
}

pub(super) fn group_id(group: Group) -> egui::Id {
    egui::Id::new(("hierarchy_scene", group))
}

pub(super) fn node_id(entity_id: u32) -> egui::Id {
    egui::Id::new(("hierarchy_node", entity_id))
}

/// A new branch starts open unless it is a skeleton bone; the default is stored, as
/// `is_bone` walks the ancestors each time.
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
    headers: usize,
}

impl Flattener {
    pub fn flatten(&mut self, ctx: &egui::Context, scene: &Scene) {
        self.rows.clear();
        self.kids.clear();
        self.headers = 0;
        let handles = scene.loaded.handles();
        if handles.is_empty() {
            for &id in scene.world.ids() {
                if scene.world.parent_id(id).is_none() {
                    self.walk(ctx, scene, id, 0, None);
                }
            }
            return;
        }
        let mut groups: Vec<Group> = handles.into_iter().map(Group::Scene).collect();
        if !scene.loaded.persistent.is_empty() {
            groups.push(Group::DontDestroyOnLoad);
        }
        for group in groups {
            self.group(ctx, scene, group);
        }
    }

    fn group(&mut self, ctx: &egui::Context, scene: &Scene, group: Group) {
        let member = match group {
            Group::Scene(handle) => Member::Scene(Some(handle)),
            Group::DontDestroyOnLoad => Member::DontDestroyOnLoad,
        };
        let id = group_id(group);
        let open = CollapsingState::load_with_default_open(ctx, id, true).is_open();
        let index = self.rows.len();
        self.headers += 1;
        self.rows.push(Row {
            id: 0,
            depth: 0,
            open: Some(open),
            parent: None,
            end: index + 1,
            group: Some(group),
        });
        if open {
            for &id in scene.world.ids() {
                let root = scene.world.parent_id(id).is_none();
                if root && scene.member_of(id) == Some(member) {
                    self.walk(ctx, scene, id, 1, Some(index));
                }
            }
        }
        self.rows[index].end = self.rows.len();
    }

    fn walk(
        &mut self,
        ctx: &egui::Context,
        scene: &Scene,
        id: u32,
        depth: usize,
        parent: Option<usize>,
    ) {
        // Past one row per entity the `children` lists hold a cycle: end the walk,
        // not the stack.
        let entities = self.rows.len() - self.headers;
        if scene.world.name(id).is_none() || entities >= scene.world.len() {
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
            group: None,
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
