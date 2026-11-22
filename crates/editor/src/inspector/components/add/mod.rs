use egui_phosphor::regular as icon;

use rusty_components::registry::first_class;
use rusty_edit::authoring::{self, ComponentKind};
use rusty_edit::DEFAULT_SCRIPTS_DEST_DIR;

/// 3F. Add Component — Unity-style full-width pill that opens the component menu.
pub fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, is_dirty: &mut bool) {
    ui.add_space(crate::theme::from_ui(ui).space_sm);
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
fn add_menu(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) -> bool {
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
/// (empty for none). Implemented beside the component's
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

fn add_script_menu(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) -> bool {
    let scripts = rusty_script::scripting::monobehaviour_scripts(DEFAULT_SCRIPTS_DEST_DIR);
    if scripts.is_empty() {
        ui.add_enabled(
            false,
            egui::Button::new(format!("{}  No script behaviours found", icon::MOON)),
        );
        return false;
    }
    let mut added = false;
    for path in scripts {
        let label = rusty_script::scripting::script_label(&path);
        // Each pick appends: an entity may hold one script twice, as in Unity.
        if ui.button(format!("{}  {}", icon::MOON, label)).clicked() {
            added |= authoring::add_script(world, id, &path, Default::default());
            ui.close();
        }
    }
    added
}
