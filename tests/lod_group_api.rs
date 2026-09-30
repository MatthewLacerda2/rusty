//! Integration coverage for the `LODGroup` namespace (#472): every setter writes
//! through the shared ops (ordered, clamped, out-of-range levels ignored), the
//! getters read it back, the component is addable by name and survives a scene
//! save → load, and its renderer slots are recognised as entity references.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{Entity, LodGroupComponent, LodLevel};
use rusty::scene::authoring::{add_component, ComponentKind};
use rusty::scene::Scene;

fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::lod_group::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

fn eval(lua: &Lua, code: &str) -> String {
    let v: mlua::MultiValue = lua.load(code).eval().unwrap();
    let parts: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
    parts.join(", ")
}

#[test]
fn setters_write_through_and_getters_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Crate".to_string());
    assert!(add_component(
        &mut scene,
        id,
        ComponentKind::parse("LODGroup").unwrap()
    ));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "LODGroup.SetSize({id}, 2.5)
             LODGroup.SetRenderers({id}, 0, {{7, 8}})
             LODGroup.SetRenderers({id}, 1, {{9}})
             local added = LODGroup.AddLevel({id})
             LODGroup.SetLevelHeight({id}, added, 0.9)
             LODGroup.SetLevelHeight({id}, 0, 0.6)
             LODGroup.SetRenderers({id}, 5, {{1}})"
        ))
        .exec()
        .unwrap();
        let got = eval(
            lua,
            &format!(
                "return LODGroup.GetSize({id}), LODGroup.GetLevelCount({id}),
                 LODGroup.GetLevelHeight({id}, 0), LODGroup.GetLevelHeight({id}, 2),
                 #LODGroup.GetRenderers({id}, 0), LODGroup.GetLevelHeight({id}, 9),
                 LODGroup.GetLevelCount(99), #LODGroup.GetRenderers(99, 0)"
            ),
        );
        // The added level's 0.9 is clamped to its finer neighbour's 1% (the default
        // coarsest threshold).
        let expected = "Number(2.5), Integer(3), Number(0.6000000238418579), \
                        Number(0.009999999776482582), Integer(2), Nil, Integer(0), Integer(0)";
        assert_eq!(got, expected);
        lua.load(format!("LODGroup.RemoveLevel({id}, 0)"))
            .exec()
            .unwrap();
        let first = eval(lua, &format!("return LODGroup.GetRenderers({id}, 0)[1]"));
        assert_eq!(first, "Integer(9)", "LOD1 became LOD0");
    });
}

#[test]
fn an_lod_group_survives_save_and_load() {
    let mut scene = Scene::new();
    let group = scene.add_entity("Crate".to_string());
    let lod0 = scene.add_entity("Crate_LOD0".to_string());
    let lod1 = scene.add_entity("Crate_LOD1".to_string());
    let level = |screen_height, r| LodLevel {
        screen_height,
        renderers: vec![r],
    };
    let lod = LodGroupComponent {
        levels: vec![level(0.4, lod0), level(0.05, lod1)],
        size: 3.0,
    };
    scene.world.set_lod_group(group, Some(lod.clone()));
    let path = std::env::temp_dir()
        .join(format!("rusty_lod_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.world.lod_group(group).map(|g| g.clone()), Some(lod));
}

#[test]
fn renderer_slots_are_entity_references() {
    assert!(Entity::is_ref_pointer("/lod_group/levels/1/renderers/0"));
    assert!(!Entity::is_ref_pointer("/lod_group/levels/1/screen_height"));
    assert!(!Entity::is_ref_pointer("/lod_group/size"));
}
