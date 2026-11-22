use mlua::Lua;

const OBJ: &str = "mtllib pane.mtl\no Pane\n\
v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\n\
v 0 0 0.05\nv 0 1 0.05\nv 1 1 0.05\nv 1 0 0.05\n\
vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\nvn 0 0 -1\n\
usemtl Steel\nf 1/1/1 2/2/1 3/3/1\nf 1/1/1 3/3/1 4/4/1\n\
usemtl Glass\nf 5/1/2 6/2/2 7/3/2\nf 5/1/2 7/3/2 8/4/2\n";

/// A Lua state whose `bake(settings)` bakes the steel-and-glass pair's `ao` and returns its mean.
fn lua(name: &str) -> Lua {
    let dir = rusty_test_support::temp::dir().join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("pane.obj"), OBJ).unwrap();
    std::fs::write(dir.join("pane.mtl"), "newmtl Steel\nnewmtl Glass\n").unwrap();
    let lua = Lua::new();
    super::super::register(&lua).unwrap();
    let model = dir.join("pane.obj").to_str().unwrap().to_string();
    lua.globals().set("MODEL", model).unwrap();
    let mean = lua
        .create_function(|_, path: String| {
            let img = image::open(&path).unwrap().into_luma16();
            let sum: f64 = img.pixels().map(|p| f64::from(p[0]) / 65535.0).sum();
            Ok(sum / f64::from(img.width() * img.height()))
        })
        .unwrap();
    lua.globals().set("mean", mean).unwrap();
    let prefix = format!("{}/pane_", dir.to_str().unwrap());
    lua.globals().set("PREFIX", prefix).unwrap();
    lua.load("function bake(s) s.resolution = 16 s.ao_rays = 16 return mean(Texture.BakeMeshMaps(MODEL, { 'ao' }, PREFIX, s).ao) end")
        .exec()
        .unwrap();
    lua
}

#[test]
fn a_material_bakes_its_own_atlas_and_an_ignored_one_casts_no_occlusion() {
    let lua = lua("mesh_maps_faces");
    let mean = |s: &str| lua.load(format!("return bake({s})")).eval::<f64>().unwrap();
    assert!(
        mean("{ material = 'Steel' }") < 0.3,
        "the glass shades the steel"
    );
    assert_eq!(
        mean("{ material = { 'Steel' }, ignore_occluders = { 'Glass' } }"),
        1.0
    );
    assert_eq!(
        mean("{ ignore_occluders = { 'Glass' } }"),
        1.0,
        "only the steel writes"
    );
}

#[test]
fn a_material_the_model_lacks_is_an_error_naming_its_materials() {
    let lua = lua("mesh_maps_faces_bad");
    let err = |s: &str| {
        let e = lua
            .load(format!("return bake({s})"))
            .eval::<f64>()
            .unwrap_err();
        e.to_string()
    };
    let e = err("{ material = 'Glas' }");
    assert!(
        e.contains("has no material \"Glas\" (its materials: Steel, Glass)"),
        "{e}"
    );
    let e = err("{ ignore_occluders = 'Chrome' }");
    assert!(
        e.contains("Texture.BakeMeshMaps:") && e.contains("\"Chrome\""),
        "{e}"
    );
    let e = err("{ material = 3 }");
    assert!(
        e.contains("material is a material name or a list of them"),
        "{e}"
    );
}
