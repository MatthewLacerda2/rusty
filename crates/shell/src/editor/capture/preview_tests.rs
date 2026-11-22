//! `--preview`'s arguments: the mesh name and the selection it needs.

use super::*;

#[test]
fn a_mesh_name_parses_in_any_case() {
    assert_eq!(parse_mesh("plane"), Ok(PreviewMesh::Plane));
    assert_eq!(parse_mesh("SUZANNE"), Ok(PreviewMesh::Suzanne));
}

#[test]
fn an_unknown_mesh_is_an_error_listing_the_meshes() {
    let err = parse_mesh("Torus").unwrap_err();
    assert!(err.contains("\"Torus\""), "{err}");
    for mesh in PreviewMesh::ALL {
        assert!(err.contains(mesh.name()), "{err} lacks {}", mesh.name());
    }
}

#[test]
fn preview_beside_the_picker_is_refused() {
    let err = refuse_with_picker(Some(PreviewMesh::Cube)).unwrap_err();
    assert!(err.contains("--picker"), "{err}");
    assert_eq!(refuse_with_picker(None), Ok(()));
}

/// The default scene: `Player` has a Material, `Sun` none.
fn scene() -> Scene {
    let mut scene = Scene::new();
    rusty_edit::default_scene::build(&mut scene, rusty_edit::default_scene::BOT_SCRIPT);
    scene
}

/// A selection applied to a fresh editor before `open`.
type Select = Box<dyn FnOnce(&mut EditorUi, &Scene)>;

/// `open` over a fresh editor with `select` applied first.
fn open_on(select: impl FnOnce(&mut EditorUi, &Scene)) -> (EditorUi, Result<(), String>) {
    let (mut ui, scene) = (EditorUi::new(Default::default()), scene());
    select(&mut ui, &scene);
    let result = open(&mut ui, &scene, Some(PreviewMesh::Plane));
    (ui, result)
}

#[test]
fn a_card_without_the_tab_is_an_error_and_the_tab_stays_shut() {
    let named = |name: &'static str| {
        move |ui: &mut EditorUi, scene: &Scene| {
            ui.selected_entity_id = scene.find_entity_by_name(name);
        }
    };
    let asset = |path: &'static str| {
        move |ui: &mut EditorUi, _: &Scene| ui.selected_asset_path = Some(path.to_string())
    };
    let cases: [(&str, Select); 3] = [
        (
            "nothing is selected",
            Box::new(|_: &mut EditorUi, _: &Scene| {}),
        ),
        ("has no Material", Box::new(named("Sun"))),
        ("has no preview", Box::new(asset("sounds/hit.wav"))),
    ];
    for (why, select) in cases {
        let (ui, result) = open_on(select);
        let err = result.unwrap_err();
        assert!(err.contains(why) && err.contains("no Preview tab"), "{err}");
        assert!(!ui.preview_tab_active, "a refused tab stays shut");
    }
}

#[test]
fn preview_opens_the_tab_on_the_mesh() {
    let named = |ui: &mut EditorUi, scene: &Scene| {
        ui.selected_entity_id = scene.find_entity_by_name("Player");
    };
    for select in [
        Box::new(named) as Select,
        Box::new(|ui: &mut EditorUi, _: &Scene| ui.selected_asset_path = Some("a.png".into())),
    ] {
        let (ui, result) = open_on(select);
        result.expect("opens");
        assert!(ui.preview_tab_active);
        assert_eq!(ui.preview_mesh, PreviewMesh::Plane);
    }
    // Without the flag nothing changes, whatever is selected.
    let mut ui = EditorUi::new(Default::default());
    open(&mut ui, &scene(), None).expect("no-op");
    assert!(!ui.preview_tab_active);
}
