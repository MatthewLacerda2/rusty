use std::path::{Path, PathBuf};

use mlua::Lua;

use super::*;

const L_OBJ: &str = "\
v 0 0 0\nv 0 0 1\nv 1 0 1\nv 1 0 0\nv 0 1 0\nv 0 1 1\n\
vt 0.05 0.05\nvt 0.45 0.05\nvt 0.45 0.95\nvt 0.05 0.95\n\
vt 0.55 0.05\nvt 0.95 0.05\nvt 0.95 0.95\nvt 0.55 0.95\n\
vn 0 1 0\nvn 1 0 0\n\
f 1/1/1 2/2/1 3/3/1\nf 1/1/1 3/3/1 4/4/1\n\
f 1/5/2 5/6/2 6/7/2\nf 1/5/2 6/7/2 2/8/2\n";

fn project(name: &str) -> (PathBuf, String) {
    let dir = rusty_test_support::temp::dir().join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("l.obj");
    std::fs::write(&model, L_OBJ).unwrap();
    (dir, model.to_str().unwrap().to_string())
}

fn all() -> Vec<String> {
    MeshMap::ALL.map(|m| m.name().to_string()).to_vec()
}

#[test]
fn every_map_is_written_beside_the_prefix_in_a_new_folder() {
    let (dir, model) = project("mesh_maps_all");
    let prefix = format!("{}/maps/l_", dir.to_str().unwrap());
    let written = bake_mesh_maps(&model, &all(), &prefix, &Settings::new(16), &dir).unwrap();
    assert_eq!(written.len(), MeshMap::ALL.len());
    for (map, path) in &written {
        assert_eq!(path, &format!("{prefix}{map}.png"));
        let img = image::open(path).expect("a map decodes");
        assert_eq!(image::GenericImageView::dimensions(&img), (16, 16), "{map}");
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_prefix_ending_in_a_slash_gets_its_folder_made() {
    let (dir, model) = project("mesh_maps_slash");
    let prefix = format!("{}/maps/kolvik/", dir.to_str().unwrap());
    let written = bake_mesh_maps(
        &model,
        &["id".to_string()],
        &prefix,
        &Settings::new(8),
        &dir,
    )
    .unwrap();
    assert!(Path::new(&written["id"]).is_file(), "{written:?}");
    assert!(written["id"].ends_with("/maps/kolvik/id.png"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_bad_call_names_the_verb_and_the_model() {
    let (dir, model) = project("mesh_maps_bad");
    let prefix = format!("{}/l_", dir.to_str().unwrap());
    let bad = |model: &str, maps: &[String]| {
        bake_mesh_maps(model, maps, &prefix, &Settings::new(8), &dir).unwrap_err()
    };
    let e = bad(&model, &["dirt".to_string()]);
    assert!(
        e.starts_with(&format!("Texture.BakeMeshMaps: {model}:")),
        "{e}"
    );
    assert!(e.contains("\"dirt\""), "{e}");
    assert!(bad(&model, &[]).contains("no maps"));
    assert!(bad(&format!("{model}::Nope"), &all()).contains("no sub-object \"Nope\""));
    assert!(bad(&format!("{model}.missing.obj"), &all()).contains("missing"));
    assert!(
        !Path::new(&format!("{prefix}ao.png")).exists(),
        "nothing written"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_recipe_reads_a_baked_map_through_the_image_op() {
    let (dir, model) = project("mesh_maps_recipe");
    let lua = Lua::new();
    super::super::register(&lua).unwrap();
    lua.globals().set("MODEL", model).unwrap();
    lua.globals()
        .set("PREFIX", format!("{}/l_", dir.to_str().unwrap()))
        .unwrap();
    lua.globals()
        .set("OUT", dir.join("wear.png").to_str().unwrap())
        .unwrap();
    let script = r#"
        local maps = Texture.BakeMeshMaps(MODEL, { "curvature", "ao" }, PREFIX, 32)
        return Texture.Bake({ resolution = 32, nodes = {
          { id = "c", op = "image", path = maps.curvature, encoding = "linear" },
        } }, OUT, "data")
    "#;
    let out: String = lua.load(script).eval().unwrap();
    let img = image::open(&out).unwrap().to_luma16();
    // The floor's texel beside the wall sits in the concave crease: darker than 0.5.
    // u ≈ 0.39, v ≈ 0.92 (the .obj flips v): the floor, by its x = 0 edge.
    let (x, y) = (12, 29);
    assert!(img.get_pixel(x, y)[0] < 30_000, "{:?}", img.get_pixel(x, y));
    std::fs::remove_dir_all(dir).ok();
}

fn settings_of(src: &str) -> Result<Settings, String> {
    settings(&Lua::new().load(src).eval::<mlua::Value>().unwrap())
}

#[test]
fn nil_a_number_and_a_resolution_field_are_the_same_settings() {
    assert_eq!(
        settings_of("return nil"),
        Ok(Settings::new(DEFAULT_RESOLUTION))
    );
    assert_eq!(settings_of("return 64"), Ok(Settings::new(64)));
    assert_eq!(
        settings_of("return { resolution = 64 }"),
        Ok(Settings::new(64))
    );
    assert_eq!(
        settings_of("return {}"),
        Ok(Settings::new(DEFAULT_RESOLUTION))
    );
}

#[test]
fn a_settings_table_names_size_band_and_rays() {
    let s = settings_of(
        "return { size = {64, 16}, curvature_radius = 0.003, ao_rays = 256, thickness_rays = 8 }",
    )
    .unwrap();
    assert_eq!(s.size, rusty_procgen::image_buf::Size::new(64, 16));
    assert_eq!(s.curvature_radius, Some(0.003));
    assert_eq!((s.ao_rays, s.thickness_rays), (256, 8));
    for (bad, says) in [
        ("return { resolution = 8, size = {8, 8} }", "both"),
        ("return { ao_ray = 8 }", "ao_ray"),
        ("return \"big\"", "string"),
        ("return -4", "resolution -4"),
        ("return { size = {0, 0} }", "size {0, 0}"),
        ("return { size = {16, 9000} }", "size {16, 9000}"),
    ] {
        let e = settings_of(bad).unwrap_err();
        assert!(e.contains(says), "{bad}: {e}");
    }
}

#[test]
fn a_non_square_bake_writes_that_shape_and_the_baker_checks_the_ranges() {
    let (dir, model) = project("mesh_maps_settings");
    let lua = Lua::new();
    super::super::register(&lua).unwrap();
    lua.globals().set("MODEL", model).unwrap();
    lua.globals()
        .set("PREFIX", format!("{}/l_", dir.to_str().unwrap()))
        .unwrap();
    let script = r#"return Texture.BakeMeshMaps(MODEL, { "ao" }, PREFIX,
        { size = { 32, 8 }, ao_rays = 4 }).ao"#;
    let path: String = lua.load(script).eval().unwrap();
    let img = image::open(&path).unwrap();
    assert_eq!(image::GenericImageView::dimensions(&img), (32, 8));
    let bad = r#"return Texture.BakeMeshMaps(MODEL, { "ao" }, PREFIX, { ao_rays = 0 })"#;
    let e = lua.load(bad).eval::<mlua::Value>().unwrap_err().to_string();
    assert!(
        e.contains("Texture.BakeMeshMaps:") && e.contains("ao_rays 0"),
        "{e}"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_radius_the_texels_cannot_hold_is_told_once_per_bake() {
    rusty_test_support::logs::init();
    let (dir, model) = project("mesh_maps_floor");
    let prefix = format!("{}/l_", dir.to_str().unwrap());
    let told = |radius: f32, maps: &[&str]| {
        rusty_test_support::logs::take();
        let maps: Vec<String> = maps.iter().map(|m| m.to_string()).collect();
        let s = Settings {
            curvature_radius: Some(radius),
            ..Settings::new(16)
        };
        bake_mesh_maps(&model, &maps, &prefix, &s, &dir).unwrap();
        rusty_test_support::logs::take()
            .into_iter()
            .filter(|m| m.contains("BakeMeshMaps") && m.contains("UV islands"))
            .collect::<Vec<_>>()
    };
    // At 16² a texel of the L's 1 m faces is ~7 cm: 1 mm is far under both floors.
    let raised = told(0.001, &["curvature", "ao"]);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(
        raised[0].contains(&format!("{model}: curvature_radius 0.00100 ")),
        "{raised:?}"
    );
    assert!(raised[0].contains("on 2 of 2 UV islands"), "{raised:?}");
    assert!(told(0.5, &["curvature"]).is_empty(), "a radius both hold");
    assert!(told(0.001, &["ao"]).is_empty(), "no curvature map");
    std::fs::remove_dir_all(dir).ok();
}
