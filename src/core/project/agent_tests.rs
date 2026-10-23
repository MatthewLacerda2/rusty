//! The agent files an edit-open writes at the project root (#1313).

use super::agent::{write, write_for};
use super::*;

pub(super) fn fresh(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join("project_agent").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn root_entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_new_project_gets_claude_md_and_in_dev_mcp_json_and_nothing_else() {
    let (opened, lines) = prepare(&fresh("new").join("Horde"), Access::Edit).unwrap();
    let root = &opened.root;
    let claude = std::fs::read_to_string(root.join(CLAUDE_FILE)).unwrap();
    assert!(claude.starts_with("# Horde\n"), "{claude}");
    assert!(claude.contains("rusty://game-guide.md"), "{claude}");
    assert!(claude.contains("cargo build --bin session-mcp"), "{claude}");
    let engine = crate::core::paths::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")));
    let engine = engine.unwrap().to_string_lossy().into_owned();
    assert!(claude.contains(&engine), "names the checkout");
    assert!(
        claude.contains("MatthewLacerda2/rusty"),
        "where findings go"
    );
    assert!(claude.trim_end().ends_with("## This game"), "{claude}");
    assert!(
        lines.contains(&format!("created {CLAUDE_FILE}")),
        "{lines:?}"
    );
    let dev = cfg!(feature = "dev");
    assert_eq!(
        root.join(MCP_FILE).is_file(),
        dev,
        "only a dev build writes it"
    );
    let mut expected = vec![ASSETS_DIR, CACHE_DIR, CLAUDE_FILE, PROJECT_FILE, SAVED_DIR];
    if dev {
        expected.push(MCP_FILE);
    }
    expected.sort_unstable();
    assert_eq!(
        root_entries(root),
        expected,
        "the rest of the root is the user's"
    );
}

#[test]
fn an_existing_claude_md_is_left_byte_identical() {
    let root = fresh("existing");
    let own = "# My game\n\nOur own rules.\n";
    std::fs::write(root.join(CLAUDE_FILE), own).unwrap();
    prepare(&root, Access::Edit).unwrap();
    write(&root, true);
    let after = std::fs::read_to_string(root.join(CLAUDE_FILE)).unwrap();
    assert_eq!(after, own);
}

#[test]
fn a_run_open_writes_neither() {
    let (opened, _) = prepare(&fresh("run"), Access::Run).unwrap();
    assert!(!opened.root.join(CLAUDE_FILE).exists());
    assert!(!opened.root.join(MCP_FILE).exists());
}

#[test]
fn a_project_inside_the_engine_checkout_gets_neither() {
    let engine = fresh("engine");
    let root = engine.join("project");
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(write_for(&root, true, &engine), Vec::<String>::new());
    assert_eq!(root_entries(&root), Vec::<String>::new());
}

#[test]
fn a_failed_write_is_a_warning_never_a_failed_open() {
    let root = fresh("unwritable");
    let missing = root.join("gone");
    let lines = write_for(&missing, true, Path::new("/engine"));
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines.iter().all(|l| l.starts_with("warning: ")),
        "{lines:?}"
    );
    // A directory where `.mcp.json` goes can't be written, even by root.
    std::fs::create_dir(root.join(MCP_FILE)).unwrap();
    let (opened, lines) = prepare(&root, Access::Edit).expect("the open succeeds");
    assert!(opened.root.join(CLAUDE_FILE).is_file());
    if cfg!(feature = "dev") {
        let warned = format!("warning: {MCP_FILE} not written");
        assert!(lines.iter().any(|l| l.starts_with(&warned)), "{lines:?}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_root_skips_mcp_json_with_a_warning() {
    use std::os::unix::ffi::OsStrExt;
    let root = fresh("bytes").join(std::ffi::OsStr::from_bytes(b"game\xff"));
    std::fs::create_dir_all(&root).unwrap();
    let lines = write_for(&root, true, Path::new("/engine"));
    assert!(root.join(CLAUDE_FILE).is_file());
    assert!(!root.join(MCP_FILE).exists());
    assert!(lines[1].contains("not UTF-8"), "{lines:?}");
}
