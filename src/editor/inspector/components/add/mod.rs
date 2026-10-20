//! src/editor/inspector/components/add/mod.rs — the Add Component menu: the
//! project's script behaviours, then one table of first-class components (#939).

use egui_phosphor::regular as icon;

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
    for &(kind, glyph, name) in ENTRIES {
        if authoring::has_kind(world, id, kind) {
            continue;
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

/// The first-class entries of the Add Component menu, in menu order: the kind, its
/// glyph (empty for none yet, the burn-down in #82) and its label. Each is offered
/// only when absent, and picking one goes through the shared dependency verb, so the
/// menu adds exactly what `Scene.AddComponent` adds, requirements included (#359).
/// Material stages its default library asset on the entity; the inspector folds it
/// into `scene.materials` once the entity guard drops.
const ENTRIES: &[(ComponentKind, &str, &str)] = &[
    (ComponentKind::Light, "", "Light Component"),
    (ComponentKind::Animator, "", "Animator Component"),
    (ComponentKind::Collider, "", "Collider Component"),
    (ComponentKind::RigidBody, "", "RigidBody Component"),
    (ComponentKind::Texture, "", "Material / Texture Component"),
    (ComponentKind::NavMeshAgent, "", "NavMesh Agent Component"),
    (
        ComponentKind::NavMeshObstacle,
        icon::TRAFFIC_CONE,
        "NavMesh Obstacle",
    ),
    (
        ComponentKind::OffMeshLink,
        icon::LADDER_SIMPLE,
        "Off-Mesh Link",
    ),
    (
        ComponentKind::NavMeshModifierVolume,
        icon::SELECTION,
        "NavMesh Modifier Volume",
    ),
    (ComponentKind::Joint, icon::LINK, "Joint"),
    (
        ComponentKind::CharacterController,
        icon::PERSON_SIMPLE_WALK,
        "Character Controller",
    ),
    (ComponentKind::Camera, "", "Camera Component"),
    (ComponentKind::Particles, icon::SPARKLE, "Particle System"),
    (ComponentKind::Trail, icon::WIND, "Trail Renderer"),
    (ComponentKind::Line, icon::LINE_SEGMENTS, "Line Renderer"),
    (
        ComponentKind::VisualCorrection,
        "",
        "Visual Correction Component",
    ),
    (ComponentKind::LodGroup, icon::STACK_SIMPLE, "LOD Group"),
    (ComponentKind::Audio, icon::SPEAKER_HIGH, "Audio Source"),
    (
        ComponentKind::ReverbZone,
        icon::WAVEFORM,
        "Audio Reverb Zone",
    ),
    // The in-game UI (#417–#421, #425, #426, #428): a Canvas makes the entity a UI
    // root, a RectTransform a UI element; the graphics, masks, Selectable and the
    // layout pair require a RectTransform.
    (ComponentKind::Canvas, icon::MONITOR, "Canvas"),
    (
        ComponentKind::RectTransform,
        icon::FRAME_CORNERS,
        "Rect Transform",
    ),
    (ComponentKind::Image, icon::IMAGE, "Image"),
    (ComponentKind::CanvasGroup, icon::STACK, "Canvas Group"),
    (ComponentKind::Text, icon::TEXT_T, "Text"),
    (ComponentKind::Shape, icon::SHAPES, "Shape"),
    (ComponentKind::RectMask, icon::CROP, "Rect Mask"),
    (ComponentKind::Mask, icon::CIRCLE_DASHED, "Mask"),
    (
        ComponentKind::BackdropFilter,
        icon::DROP_HALF,
        "Backdrop Filter",
    ),
    (ComponentKind::Selectable, icon::CURSOR_CLICK, "Selectable"),
    (ComponentKind::LayoutGroup, icon::LAYOUT, "Layout Group"),
    (
        ComponentKind::LayoutElement,
        icon::ARROWS_OUT,
        "Layout Element",
    ),
];

/// List the project's MonoBehaviour scripts (any `.lua` exposing a lifecycle
/// table) under the moon glyph; picking one appends a `ScriptComponent` to the
/// entity's `scripts` (#83). Duplicates are allowed, mirroring Unity. When the
/// project has no such scripts, a disabled hint is shown instead. Returns whether
/// a script was added.
fn add_script_menu(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) -> bool {
    let scripts = crate::scripting::monobehaviour_scripts(DEFAULT_SCRIPTS_DEST_DIR);
    if scripts.is_empty() {
        ui.add_enabled(
            false,
            egui::Button::new(format!("{}  No script behaviours found", icon::MOON)),
        );
        return false;
    }
    let mut added = false;
    for path in scripts {
        let label = crate::scripting::script_label(&path);
        if ui.button(format!("{}  {}", icon::MOON, label)).clicked() {
            added |= authoring::add_script(world, id, &path, Default::default());
            ui.close();
        }
    }
    added
}
