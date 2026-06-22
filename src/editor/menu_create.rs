//! src/editor/menu_create.rs — the **GameObject** menu (Unity-style authoring).
//!
//! The single, discoverable home for creating objects, mirroring Unity's
//! `GameObject` menu: **Create Empty**, **3D Object ▸** (Cube / Sphere / Plane /
//! Cylinder) and **Light ▸** (Directional / Point / Spot). Object creation used to
//! live in the Hierarchy toolbar (#255); it moved here so the Hierarchy does one
//! job (show + select the tree).
//!
//! Every entry routes through the SAME shared create path the `Scene.CreateEntity`
//! API uses (`scene::authoring::create_entity`), so editor and API can never drift:
//! there is no second creation path, just a new entry point that then selects the
//! new entity and marks the scene dirty.

use egui_phosphor::regular as icon;

use crate::editor::EditorUi;
use crate::scene::authoring::{self, Primitive};
use crate::scene::Scene;

/// Draw the top-level **GameObject** menu button into the menu bar.
pub fn game_object_menu(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    ui.menu_button(format!("{}  GameObject", icon::CUBE), |ui| {
        if ui
            .button(format!("{}  Create Empty", icon::CUBE_TRANSPARENT))
            .clicked()
        {
            spawn(editor, scene, "GameObject", None);
            ui.close_menu();
        }

        ui.menu_button(format!("{}  3D Object", icon::CUBE), |ui| {
            for (label, glyph, primitive) in [
                ("Cube", icon::CUBE, Primitive::Box),
                ("Sphere", icon::CIRCLE, Primitive::Sphere),
                ("Plane", icon::RECTANGLE, Primitive::Plane),
                ("Cylinder", icon::CYLINDER, Primitive::Cylinder),
            ] {
                if ui.button(format!("{glyph}  {label}")).clicked() {
                    spawn(editor, scene, label, Some(primitive));
                    ui.close_menu();
                }
            }
        });

        ui.menu_button(format!("{}  Light", icon::LIGHTBULB), |ui| {
            for (label, glyph, primitive) in [
                ("Directional Light", icon::SUN, Primitive::DirectionalLight),
                ("Point Light", icon::LIGHTBULB, Primitive::PointLight),
                ("Spot Light", icon::FLASHLIGHT, Primitive::SpotLight),
            ] {
                if ui.button(format!("{glyph}  {label}")).clicked() {
                    spawn(editor, scene, label, Some(primitive));
                    ui.close_menu();
                }
            }
        });
    });
}

/// Create an entity through the shared authoring path, then select it and mark the
/// scene dirty — the exact effect the old Hierarchy "Create" button had.
fn spawn(editor: &mut EditorUi, scene: &mut Scene, name: &str, primitive: Option<Primitive>) {
    let new_id = authoring::create_entity(scene, name, primitive);
    editor.selected_entity_id = Some(new_id);
    editor.selected_asset_path = None; // entity selection is exclusive of asset selection
    editor.is_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    // The menu maps each label to the matching `Primitive`; verify the mapping
    // creates the configured component via the shared authoring path (the UI clicks
    // themselves are exercised by hand — egui has no headless click harness here).
    #[test]
    fn spawn_routes_through_authoring() {
        let mut editor = EditorUi::new();
        let mut scene = Scene::new();

        spawn(&mut editor, &mut scene, "GameObject", None);
        let empty = editor.selected_entity_id.expect("empty selected");
        // Read into owned values so the entity guard drops before the next spawn
        // (a live `Ref` would hold an immutable borrow of `scene`).
        let empty_is_bare = {
            let e = scene.get_entity(empty).expect("empty exists");
            e.mesh.is_none() && e.light.is_none()
        };
        assert!(empty_is_bare, "empty = transform only");
        assert!(editor.is_dirty, "creation marks the scene dirty");

        spawn(&mut editor, &mut scene, "Cube", Some(Primitive::Box));
        let cube = editor.selected_entity_id.expect("cube selected");
        let mesh_name = scene
            .get_entity(cube)
            .and_then(|e| e.mesh.as_ref().map(|m| m.primitive_type.clone()))
            .expect("cube gets a mesh");
        assert_eq!(mesh_name, "Box");
        assert_eq!(
            editor.selected_asset_path, None,
            "entity selection clears asset"
        );
    }
}
