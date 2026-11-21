use super::{Pass, POST_EFFECTS};

const SCENE_SIZES: [&str; 2] = ["full_size", "scene_size("];

fn scene_sized(source: &str) -> Vec<String> {
    let lines = source.lines().filter(|l| !l.contains("chain-size: scene "));
    let hits = lines.filter(|l| SCENE_SIZES.iter().any(|name| l.contains(name)));
    hits.map(|l| l.trim().to_string()).collect()
}

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
