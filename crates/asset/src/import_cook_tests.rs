//! The import pass's cooked files (#2332): it decides a model's by the read a load
//! makes, so a file that read refuses is written again, stamp or not.

use super::import_all;
use std::path::Path;

/// `model`'s cooked file, cooked by an import pass over its folder, then made
/// over by `spoil`: the pass after must write it again, and a load then read it.
fn rewritten_after(name: &str, spoil: impl FnOnce(&Path)) {
    let model = crate::cook::fixture::model(name);
    let root = model.parent().unwrap();
    let pass = || crate::cook::with_writes(true, || import_all(root).unwrap().cooked);
    assert_eq!(pass(), 1);
    let file = crate::cook::fixture::cooked_file(&model);
    spoil(&file);
    assert_eq!(
        pass(),
        1,
        "the read refused it, so the pass cooked it again"
    );
    let cooks = crate::cook::cooks();
    crate::cook::with_writes(true, || crate::import_file(&model).unwrap());
    assert_eq!(crate::cook::cooks(), cooks, "a load reads what it wrote");
    assert_eq!(pass(), 0, "and the next pass finds it current");
}

#[test]
fn a_stamped_file_from_another_parse_is_cooked_again() {
    // What a parser change without a COOKER_VERSION bump leaves: the stamp holds.
    rewritten_after("import_all_reparsed", |file| {
        let bytes = std::fs::read(file).unwrap();
        let json = crate::import_cache::glb_json(file).unwrap();
        let root: serde_json::Value = serde_json::from_slice(&json).unwrap();
        let hex = root["asset"]["extras"]["stamp"].as_str().unwrap();
        let stamp = u64::from_str_radix(hex, 16).unwrap();
        let mut subs = crate::cook::file::parse(&bytes, stamp).unwrap();
        subs.iter_mut().for_each(|s| s.parse ^= 1);
        std::fs::write(file, crate::cook::file::encode(stamp, &subs)).unwrap();
    });
}

#[test]
fn a_cooked_file_cut_short_is_cooked_again() {
    rewritten_after("import_all_cut", |file| {
        let bytes = std::fs::read(file).unwrap();
        std::fs::write(file, &bytes[..bytes.len() - 64]).unwrap();
    });
}
