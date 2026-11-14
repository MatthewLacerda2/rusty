//! Scene-hierarchy tree rendering (VS Code Explorer style): a genuine collapsible
//! tree with per-entity expand state, type glyphs and full-row selection. Kept in
//! its own module so `hierarchy`'s root stays under the size gate.

use egui::collapsing_header::CollapsingState;
use egui_phosphor::regular as icon;

use crate::theme::Theme;
use rusty_scene::Scene;

/// What a row reads and writes besides the scene: the editor's selection fields
/// (clicking a row updates them), the row a context-menu *Delete* asked for
/// (`hierarchy::delete` applies it after the tree is drawn), and the theme.
pub(super) struct Rows<'a> {
    pub sel_entity: &'a mut Option<u32>,
    pub sel_asset: &'a mut Option<String>,
    pub delete: &'a mut Option<u32>,
    pub theme: Theme,
}

impl Rows<'_> {
    /// Draw `entity_id` and (when expanded) its subtree.
    pub(super) fn draw_node(&mut self, ui: &mut egui::Ui, scene: &Scene, entity_id: u32) {
        let Some(name) = scene.world.name(entity_id).map(|n| n.clone()) else {
            return;
        };
        let children = scene.world.children(entity_id);
        if children.is_empty() {
            // Leaf: indent to line up with the parents' twisty, then the row.
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().icon_width);
                self.row(ui, scene, entity_id, &name);
            });
            return;
        }
        // Branch: a real collapsible node. egui's body indent draws the guide line.
        let id = ui.make_persistent_id(("hierarchy_node", entity_id));
        // A skeleton is ~65 bones (#453): its subtrees start collapsed.
        let default_open = !scene.is_bone(entity_id);
        CollapsingState::load_with_default_open(ui.ctx(), id, default_open)
            .show_header(ui, |ui| self.row(ui, scene, entity_id, &name))
            .body(|ui| {
                for child in children {
                    self.draw_node(ui, scene, child);
                }
            });
    }

    /// The clickable row: type glyph + name. Selecting toggles off to None (which
    /// the inspector reads as "show scene settings"). Right-click offers "Save as
    /// Prefab", which extracts this entity's subtree to a `.prefab` asset (#215),
    /// and "Delete", which deletes it and its subtree (#1915).
    fn row(&mut self, ui: &mut egui::Ui, scene: &Scene, entity_id: u32, name: &str) {
        let (glyph, glyph_color) = node_glyph(scene, entity_id, self.theme);
        let is_selected = *self.sel_entity == Some(entity_id);
        ui.colored_label(glyph_color, glyph);
        let label = ui.selectable_label(is_selected, name);
        if label.clicked() {
            if is_selected {
                *self.sel_entity = None;
            } else {
                *self.sel_entity = Some(entity_id);
                *self.sel_asset = None;
            }
        }
        label.context_menu(|ui| {
            let prefab = format!("{}  Save as Prefab", icon::PACKAGE);
            if ui.button(prefab).clicked() {
                save_as_prefab(scene, entity_id, name);
                ui.close();
            }
            if ui.button(format!("{}  Delete", icon::TRASH)).clicked() {
                *self.delete = Some(entity_id);
                ui.close();
            }
        });
    }
}

/// Extract `entity_id`'s subtree to `assets/prefabs/{name}.prefab` via the shared
/// `scene::save_prefab` verb (the same path `Scene.SavePrefab` takes). The folder is
/// created on demand; the entity name is sanitised into a filesystem-safe stem.
fn save_as_prefab(scene: &Scene, entity_id: u32, name: &str) {
    let dir = std::path::Path::new(rusty_core::project::ASSETS_DIR).join("prefabs");
    std::fs::create_dir_all(&dir).ok();
    let stem: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = if stem.is_empty() { "Prefab" } else { &stem };
    let path = dir.join(format!("{stem}.{}", rusty_scene::PREFAB_EXTENSION));
    let _ = rusty_scene::save_prefab(scene, entity_id, &path.to_string_lossy());
}

/// How a hierarchy glyph is tinted, resolved against the theme when drawn.
#[derive(Clone, Copy)]
enum Tint {
    Muted,
    Accent,
    Warning,
    Teal,
}

type Has = fn(&rusty_ecs::World, u32) -> bool;

/// The row glyph per kind of GameObject, most telling component first (#668): a
/// row says what the object *is* — camera, light, UI, audio, character — before
/// falling back to the plain mesh cube, and an empty object gets a dashed circle.
const KINDS: &[(Has, &str, Tint)] = &[
    (|w, id| w.has_camera(id), icon::VIDEO_CAMERA, Tint::Accent),
    (|w, id| w.has_light(id), icon::LIGHTBULB, Tint::Warning),
    (|w, id| w.has_canvas(id), icon::BROWSER, Tint::Teal),
    (
        |w, id| w.has_rect_transform(id),
        icon::FRAME_CORNERS,
        Tint::Teal,
    ),
    (|w, id| w.has_particles(id), icon::SPARKLE, Tint::Warning),
    (
        |w, id| w.has_audio(id) || w.has_reverb_zone(id),
        icon::SPEAKER_HIGH,
        Tint::Teal,
    ),
    (
        |w, id| w.has_character_controller(id) || w.has_animator(id),
        icon::PERSON_SIMPLE,
        Tint::Accent,
    ),
    (
        |w, id| w.has_nav_agent(id),
        icon::NAVIGATION_ARROW,
        Tint::Accent,
    ),
    (
        |w, id| w.has_nav_obstacle(id) || w.has_nav_modifier(id) || w.has_offmesh_link(id),
        icon::PATH,
        Tint::Muted,
    ),
    (
        |w, id| w.has_trail(id) || w.has_line(id),
        icon::LINE_SEGMENTS,
        Tint::Muted,
    ),
    (|w, id| w.has_mesh(id), icon::CUBE, Tint::Muted),
    (|w, id| w.has_collider(id), icon::BOUNDING_BOX, Tint::Muted),
    (|w, id| w.has_joint(id), icon::LINK, Tint::Muted),
];

/// The row's glyph and its colour. Skeleton bones read as bones whatever they carry.
fn node_glyph(scene: &Scene, id: u32, t: Theme) -> (&'static str, egui::Color32) {
    let (glyph, tint) = if scene.is_bone(id) {
        (icon::BONE, Tint::Muted)
    } else {
        KINDS
            .iter()
            .find(|(has, _, _)| has(&scene.world, id))
            .map_or((icon::CIRCLE_DASHED, Tint::Muted), |&(_, g, tint)| {
                (g, tint)
            })
    };
    let color = match tint {
        Tint::Muted => t.text_secondary,
        Tint::Accent => t.accent,
        Tint::Warning => t.warning,
        Tint::Teal => t.teal,
    };
    (glyph, color)
}
