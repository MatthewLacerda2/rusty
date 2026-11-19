//! A load's `models_cooked` (#2332): a bench cooks in memory and writes nothing
//! into the project; after an import (`make import`) it cooks nothing.

use super::tests::project_with_a_model;
use super::*;

/// The `models_cooked` count of one load of `root`'s scene, cooked files kept for
/// a model outside a project (`cook::with_writes`), as one inside keeps them.
fn cooked_in(root: &std::path::Path) -> f64 {
    rusty_asset::cook::with_writes(true, || load_of(root))
}

fn load_of(root: &std::path::Path) -> f64 {
    let opts = Options {
        size: (32, 32),
        warm: false,
        ..Options::default()
    };
    let settings = rusty_core::project::ProjectSettings::default();
    let (_, load) = load(&root.join("out"), root, "quad", settings, opts).unwrap();
    assert_eq!(load.counts[2].0, "models_cooked");
    load.counts[2].1
}

/// The quad `project_with_a_model` instances, and where its cooked file goes.
fn quad() -> (std::path::PathBuf, std::path::PathBuf) {
    let model = std::path::PathBuf::from(rusty_test_support::models::quad_file("load_rows_quad"));
    let at = || rusty_asset::cook::cooked_path(&model);
    let cooked = rusty_asset::cook::with_writes(true, at).unwrap();
    let _ = std::fs::remove_file(&cooked);
    (model, cooked)
}

#[test]
fn gpu_a_load_cooks_in_memory_and_writes_nothing() {
    let root = project_with_a_model();
    let (_, cooked) = quad();
    assert_eq!(cooked_in(&root), 1.0);
    assert!(
        !cooked.exists(),
        "a bench reads the project, never writes it"
    );
}

#[test]
fn gpu_a_load_after_an_import_cooks_nothing() {
    let root = project_with_a_model();
    let (model, cooked) = quad();
    // `make import`, before any harness marks this thread standalone.
    let import = || rusty_asset::import::import_all(model.parent().unwrap());
    let imported = rusty_asset::cook::with_writes(true, import).unwrap();
    assert_eq!(imported.cooked, 1);
    assert!(cooked.is_file());
    rusty_asset::import_cache::forget(&model);
    assert_eq!(
        cooked_in(&root),
        0.0,
        "the cooked file is read, nothing cooked"
    );
}
