//! The agent files an edit-open writes at the project root (#1313).

use super::agent::{claude_md, mcp_json, write};
use super::*;

fn fresh(name: &str) -> PathBuf {
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
    assert!(
        claude.contains(env!("CARGO_MANIFEST_DIR")),
        "names the checkout"
    );
    assert!(
        claude.contains("MatthewLacerda2/rusty"),
        "says where findings go"
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
    write(&root, true).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join(CLAUDE_FILE)).unwrap(),
        own
    );
}

#[test]
fn mcp_json_is_rewritten_on_every_edit_open() {
    let root = fresh("rewrite");
    std::fs::write(root.join(MCP_FILE), "{ \"stale\": true }\n").unwrap();
    let lines = write(&root, true).unwrap();
    let text = std::fs::read_to_string(root.join(MCP_FILE)).unwrap();
    assert_eq!(text, mcp_json(&root));
    assert!(
        !lines.iter().any(|l| l.contains(MCP_FILE)),
        "rewritten, not created"
    );
    assert_eq!(
        write(&root, true).unwrap(),
        Vec::<String>::new(),
        "no change"
    );
}

#[test]
fn a_run_open_writes_neither() {
    let (opened, _) = prepare(&fresh("run"), Access::Run).unwrap();
    assert!(!opened.root.join(CLAUDE_FILE).exists());
    assert!(!opened.root.join(MCP_FILE).exists());
}

#[test]
fn mcp_json_registers_attach_and_headless_servers_for_claude_code() {
    let root = Path::new("/games/horde");
    let config: serde_json::Value = serde_json::from_str(&mcp_json(root)).unwrap();
    let args = |server: &str| -> Vec<String> {
        let entry = &config["mcpServers"][server];
        assert_eq!(entry["command"], "cargo", "{server}");
        serde_json::from_value(entry["args"].clone()).unwrap()
    };
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let attach = args("rusty");
    assert_eq!(attach[..3], ["run", "--manifest-path", manifest.as_str()]);
    assert!(
        attach.ends_with(&["--".into(), "--attach".into()]),
        "{attach:?}"
    );
    let headless = args("rusty-headless");
    assert!(headless.windows(2).any(|w| w == ["--bin", "session-mcp"]));
    assert!(headless.ends_with(&["--project".into(), "/games/horde".into()]));
    assert!(claude_md(root).starts_with("# horde\n"));
}
