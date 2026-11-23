use egui_phosphor::regular as icon;

use crate::EditorUi;
use rusty_edit::authoring::ui::widgets::{self, UiWidget};
use rusty_edit::authoring::{self, Primitive};
use rusty_scene::Scene;

/// Draw the top-level **GameObject** menu button into the menu bar.
pub fn game_object_menu(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    ui.menu_button("GameObject", |ui| {
        if ui
            .button(format!("{}  Create Empty", icon::CUBE_TRANSPARENT))
            .clicked()
        {
            spawn(editor, scene, "GameObject", None);
            ui.close();
        }

        ui.menu_button(format!("{}  3D Object", icon::CUBE), |ui| {
            for (label, glyph, primitive) in [
                ("Cube", icon::CUBE, Primitive::Box),
                ("Sphere", icon::CIRCLE, Primitive::Sphere),
                ("Plane", icon::RECTANGLE, Primitive::Plane),
                ("Cylinder", icon::CYLINDER, Primitive::Cylinder),
                ("Capsule", icon::PILL, Primitive::Capsule),
            ] {
                if ui.button(format!("{glyph}  {label}")).clicked() {
                    spawn(editor, scene, label, Some(primitive));
                    ui.close();
                }
            }
        });

        ui.menu_button(format!("{}  UI", icon::CURSOR_CLICK), |ui| {
            for kind in UiWidget::ALL {
                if ui.button(kind.label()).clicked() {
                    let parent = editor.selected_entity_id;
                    let id = widgets::create_ui(scene, kind, parent);
                    select_new(editor, id);
                    ui.close();
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
                    ui.close();
                }
            }
        });
    });
}

/// Create through `authoring::create_entity`, the path `Scene.CreateEntity` takes, so
/// editor and API cannot drift; then select it.
fn spawn(editor: &mut EditorUi, scene: &mut Scene, name: &str, primitive: Option<Primitive>) {
    let new_id = authoring::create_entity(scene, name, primitive);
    select_new(editor, new_id);
}

fn select_new(editor: &mut EditorUi, new_id: u32) {
    editor.selected_entity_id = Some(new_id);
    editor.selected_asset_path = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    // The menu maps each label to the matching `Primitive`; verify the mapping
    // creates the configured component via the shared authoring path.
    #[test]
    fn spawn_routes_through_authoring() {
        let mut editor = EditorUi::new(Default::default());
        let mut scene = Scene::new();

        scene.begin_edits();
        spawn(&mut editor, &mut scene, "GameObject", None);
        scene.commit_edits(None);
        let empty = editor.selected_entity_id.expect("empty selected");
        let empty_is_bare = !scene.world.has_mesh(empty) && !scene.world.has_light(empty);
        assert!(empty_is_bare, "empty = transform only");
        editor.follow_edits(&mut scene);
        assert!(editor.is_dirty, "creation marks the scene dirty");

        spawn(&mut editor, &mut scene, "Cube", Some(Primitive::Box));
        let cube = editor.selected_entity_id.expect("cube selected");
        let mesh_name = scene
            .world
            .mesh(cube)
            .map(|m| m.primitive_type.clone())
            .expect("cube gets a mesh");
        assert_eq!(mesh_name, "Box");
        assert_eq!(
            editor.selected_asset_path, None,
            "entity selection clears asset"
        );
    }
}
