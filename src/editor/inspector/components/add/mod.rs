//! src/editor/inspector/components/add/mod.rs — the Add Component menu: the
//! project's script behaviours, then one table of first-class components (#939).

use egui_phosphor::regular as icon;

use crate::components::registry::first_class;
use crate::scene::authoring::{self, ComponentKind};
use crate::scene::DEFAULT_SCRIPTS_DEST_DIR;

/// 3F. Add Component — Unity-style full-width pill that opens the component menu.
/// A pick marks the scene unsaved, as every card's Remove does (#992).
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    ui.add_space(crate::editor::theme::from_ui(ui).space_sm);
    // A justified layout stretches the menu button to the full panel width.
    ui.with_layout(
        egui::Layout::top_down_justified(egui::Align::Center),
        |ui| {
            ui.menu_button(format!("{}  Add Component", icon::PLUS), |ui| {
                *is_dirty |= add_menu(ui, world, id);
            });
        },
    );
}

/// The menu's entries; returns whether one was picked.
fn add_menu(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) -> bool {
    // Every project `.lua` that exposes a lifecycle table is a MonoBehaviour and
    // is offered here with the moon glyph (the per-type icons for the other
    // built-ins are the burn-down in #82); helper modules without a lifecycle
    // table are not. An entity can hold many scripts (#83), so each pick appends.
    let mut added = add_script_menu(ui, world, id);
    let mut group = None;
    for (category, &(kind, _, glyph, name)) in menu_order() {
        if authoring::has_kind(world, id, kind) {
            continue;
        }
        if group.replace(category) != Some(category) {
            ui.separator();
        }
        let label = if glyph.is_empty() {
            name.to_string()
        } else {
            format!("{glyph}  {name}")
        };
        if ui.button(label).clicked() {
            added |= authoring::add_with_requirements(world, id, kind);
            ui.close();
        }
    }
    added
}

/// A group of the Add Component menu, in menu order (Unity's categories).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Rendering,
    Animation,
    Physics,
    Navigation,
    Ai,
    Effects,
    Audio,
    Ui,
}

impl Category {
    /// Every category, in menu order.
    const ALL: [Category; 8] = [
        Category::Rendering,
        Category::Animation,
        Category::Physics,
        Category::Navigation,
        Category::Ai,
        Category::Effects,
        Category::Audio,
        Category::Ui,
    ];
}

/// A first-class component's Add Component entry: its group, label and glyph
/// (empty for none yet, the burn-down in #82). Implemented beside the component's
/// inspector card; the menu lists every registry row (`components::registry`), so a
/// component without one does not compile.
pub trait AddEntry {
    const CATEGORY: Category;
    const LABEL: &'static str;
    const ICON: &'static str = "";
}

/// One menu entry: the kind, its group, glyph and label.
type Entry = (ComponentKind, Category, &'static str, &'static str);

macro_rules! entries {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        /// The first-class entries of the Add Component menu, one per registry row.
        /// Each is offered only when absent, and picking one goes through the shared
        /// dependency verb, so the menu adds exactly what `Scene.AddComponent` adds,
        /// requirements included (#359). Material stages its default library asset
        /// on the entity; the inspector folds it into `scene.materials` once the
        /// entity guard drops.
        const ENTRIES: &[Entry] = &[$((
            ComponentKind::$kind,
            <$ty as AddEntry>::CATEGORY,
            <$ty as AddEntry>::ICON,
            <$ty as AddEntry>::LABEL,
        ),)*];
    };
}

first_class!(entries);

/// The entries in menu order: by [`Category`], then the registry's order.
fn menu_order() -> impl Iterator<Item = (Category, &'static Entry)> {
    Category::ALL.into_iter().flat_map(|c| {
        ENTRIES
            .iter()
            .filter(move |e| e.1 == c)
            .map(move |e| (c, e))
    })
}

/// List the project's MonoBehaviour scripts (any `.lua` exposing a lifecycle
/// table) under the moon glyph; picking one appends a `ScriptComponent` to the
/// entity's `scripts` (#83). Duplicates are allowed, mirroring Unity. When the
/// project has no such scripts, a disabled hint is shown instead. Returns whether
/// a script was added.
fn add_script_menu(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) -> bool {
    let scripts = crate::script::scripting::monobehaviour_scripts(DEFAULT_SCRIPTS_DEST_DIR);
    if scripts.is_empty() {
        ui.add_enabled(
            false,
            egui::Button::new(format!("{}  No script behaviours found", icon::MOON)),
        );
        return false;
    }
    let mut added = false;
    for path in scripts {
        let label = crate::script::scripting::script_label(&path);
        if ui.button(format!("{}  {}", icon::MOON, label)).clicked() {
            added |= authoring::add_script(world, id, &path, Default::default());
            ui.close();
        }
    }
    added
}
