//! `--preview`'s arguments: the mesh name and the selection it needs (#2239).

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
fn preview_with_nothing_selected_is_an_error() {
    let mut ui = EditorUi::new(Default::default());
    let err = open(&mut ui, Some(PreviewMesh::Plane)).unwrap_err();
    assert!(err.contains("nothing is selected"), "{err}");
    assert!(!ui.preview_tab_active, "a refused tab stays shut");
}

#[test]
fn preview_opens_the_tab_on_the_mesh() {
    let mut ui = EditorUi::new(Default::default());
    ui.selected_entity_id = Some(1);
    open(&mut ui, Some(PreviewMesh::Cube)).expect("opens");
    assert!(ui.preview_tab_active);
    assert_eq!(ui.preview_mesh, PreviewMesh::Cube);
    // Without the flag nothing changes, whatever is selected.
    let mut ui = EditorUi::new(Default::default());
    open(&mut ui, None).expect("no-op");
    assert!(!ui.preview_tab_active);
}
