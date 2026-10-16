//! The `LayoutGroup` and `LayoutElement` namespaces write through the shared ops,
//! validation included (#421).

use std::cell::RefCell;

use glam::Vec2;
use mlua::Lua;
use rusty::components::{LayoutConstraint, LayoutElementComponent, LayoutGroupComponent};
use rusty::scene::Scene;

fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::layout_group::register(&lua, scope, scene).unwrap();
        rusty::api::layout_element::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

#[test]
fn layout_group_setters_validate_through_the_shared_ops() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Grid".to_string());
    let bare = scene.add_entity("Bare".to_string());
    scene
        .world
        .set_layout_group(id, Some(LayoutGroupComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "LayoutGroup.SetKind({id}, 'grid')
             LayoutGroup.SetKind({id}, 'Diagonal')
             LayoutGroup.SetCellSize({id}, -4, 32)
             LayoutGroup.SetConstraint({id}, 'FixedColumnCount')
             LayoutGroup.SetConstraintCount({id}, 0)
             LayoutGroup.SetPadding({id}, 1, 2, 3, 4)
             LayoutGroup.SetChildForceExpand({id}, false, true)
             LayoutGroup.SetKind({bare}, 'Vertical')"
        ))
        .exec()
        .unwrap();
        let got: (String, f32, u32, f32, bool) = lua
            .load(format!(
                "local _, h = LayoutGroup.GetCellSize({id})
                 local l, b, r, t = LayoutGroup.GetPadding({id})
                 local w = LayoutGroup.GetChildForceExpand({id})
                 return LayoutGroup.GetKind({id}), h, LayoutGroup.GetConstraintCount({id}), t, w"
            ))
            .eval()
            .unwrap();
        assert_eq!(got, ("Grid".into(), 32.0, 1, 4.0, false));
        let none: String = lua
            .load(format!("return LayoutGroup.GetKind({bare})"))
            .eval()
            .unwrap();
        assert_eq!(none, "");
    });
    let g = scene.borrow().world.layout_group(id).map(|g| g.clone());
    let g = g.expect("group");
    assert_eq!(g.cell_size, Vec2::new(0.0, 32.0));
    assert_eq!(g.constraint, LayoutConstraint::FixedColumnCount);
    assert!(!scene.borrow().world.has_layout_group(bare));
}

#[test]
fn layout_element_sizes_clear_with_nil_and_fit_by_name() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Label".to_string());
    scene
        .world
        .set_layout_element(id, Some(LayoutElementComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "LayoutElement.SetPreferredSize({id}, 120, nil)
             LayoutElement.SetMinSize({id}, -1, 10)
             LayoutElement.SetFit({id}, 'unconstrained', 'PreferredSize')
             LayoutElement.SetFit({id}, 'MinSize', 'Sideways')"
        ))
        .exec()
        .unwrap();
        let got: (Option<f32>, Option<f32>, Option<f32>, String) = lua
            .load(format!(
                "local pw, ph = LayoutElement.GetPreferredSize({id})
                 local mw = LayoutElement.GetMinSize({id})
                 local _, v = LayoutElement.GetFit({id})
                 return pw, ph, mw, v"
            ))
            .eval()
            .unwrap();
        assert_eq!(got, (Some(120.0), None, None, "PreferredSize".into()));
    });
}
