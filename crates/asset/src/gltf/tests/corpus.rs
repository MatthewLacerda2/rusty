//! The `model_import` fuzz target's committed corpus, held by the test suite: each seed
//! and regression file is refused or imports valid, without a panic.

use crate::gltf::import::import_slice;

#[test]
fn every_fuzz_corpus_file_is_refused_or_imports_valid() {
    let dir = rusty_core::project::checkout().join("fuzz/corpus/model_import");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    assert!(files.len() >= 2, "the corpus is at {}", dir.display());
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        if let Ok(asset) = import_slice(&bytes) {
            assert_eq!(asset.validate(), Ok(()), "{}", path.display());
        }
    }
}
