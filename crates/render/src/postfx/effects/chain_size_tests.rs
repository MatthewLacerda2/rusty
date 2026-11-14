//! The chain-size rule as a check (#1702): every pass that runs after TAA (a
//! `Pass::Scene` ordered after TAA's, and every `Pass::Ldr`) sizes by
//! `PostFx::chain_size`, never the scene's size (`full_size`, `scene_size()`),
//! which is the wrong one only while TAA upscales the view. A source scan of each
//! such effect's own files, since every gate stays green until a view upscales.
//! A line that must read the scene's size says so inline: `chain-size: scene <why>`.

use super::{Pass, POST_EFFECTS};

/// The names that mean the scene's size.
const SCENE_SIZES: [&str; 2] = ["full_size", "scene_size("];

/// The lines of `source` that size by the scene's size without saying why.
fn scene_sized(source: &str) -> Vec<String> {
    let lines = source.lines().filter(|l| !l.contains("chain-size: scene "));
    let hits = lines.filter(|l| SCENE_SIZES.iter().any(|name| l.contains(name)));
    hits.map(|l| l.trim().to_string()).collect()
}

/// The effect's own non-test source files: `effects/<name>.rs`, the folder
/// `effects/<name>/`, or either inside a family folder (`lens/`, `grading/`).
fn sources(name: &str) -> Vec<std::path::PathBuf> {
    let root = rusty_core::project::checkout().join("crates/render/src/postfx/effects");
    let families = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.path());
    let dirs: Vec<_> = std::iter::once(root.clone())
        .chain(families.filter(|p| p.is_dir()))
        .collect();
    let mut files = Vec::new();
    for dir in dirs {
        let file = dir.join(format!("{name}.rs"));
        if file.is_file() {
            files.push(file);
        }
        let folder = dir.join(name);
        if folder.is_dir() {
            files.extend(
                std::fs::read_dir(folder)
                    .unwrap()
                    .flatten()
                    .map(|e| e.path()),
            );
        }
    }
    let own = |p: &std::path::PathBuf| {
        let stem = p.file_name().unwrap().to_string_lossy();
        p.extension().is_some_and(|x| x == "rs")
            && !stem.contains("test")
            && stem != "gpu_fixture.rs"
    };
    files.into_iter().filter(own).collect()
}

/// Whether one of an effect's passes runs after TAA's resolve, at the chain's size.
fn after_taa(passes: &[Pass], taa: u16) -> bool {
    passes.iter().any(|pass| match pass {
        Pass::Scene { order, .. } => *order > taa,
        Pass::Ldr { .. } => true,
    })
}

#[test]
fn every_pass_after_taa_sizes_by_the_chain() {
    let Some(&Pass::Scene { order: taa, .. }) = super::taa::EFFECT.passes.first() else {
        panic!("TAA's first pass is its scene pass");
    };
    let mut found = Vec::new();
    for effect in POST_EFFECTS.iter().filter(|e| after_taa(e.passes, taa)) {
        let files = sources(effect.name);
        assert!(!files.is_empty(), "no source found for `{}`", effect.name);
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            let hits = scene_sized(&source).into_iter();
            found.extend(hits.map(|l| format!("{}: {l}", file.display())));
        }
    }
    assert!(
        found.is_empty(),
        "size by `chain_size` after TAA:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scan_flags_the_scene_size_and_passes_the_chain_or_a_reason() {
    let flagged = "let size = pfx.full_size;\nlet s = view.scene_size();";
    assert_eq!(scene_sized(flagged).len(), 2);
    let passed =
        "let size = pfx.chain_size;\nlet s = pfx.full_size; // chain-size: scene reads depth";
    assert!(scene_sized(passed).is_empty());
}
