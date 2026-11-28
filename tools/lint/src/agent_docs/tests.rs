use super::*;
use crate::repo::scaffold::Repo;

fn blank(n: usize) -> String {
    "\n".repeat(n)
}

fn rules(found: &[Finding]) -> Vec<&str> {
    found.iter().map(|f| f.rule).collect()
}

const SKILL_MD_OK: &str = "---\nname: s\ndescription: Use it.\n---\n";

fn agent_md(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Use it.\nmodel: opus\n---\n")
}

#[test]
fn a_claude_md_at_its_cap_passes_and_one_line_over_fails() {
    assert!(file("CLAUDE.md", &blank(MAX_CLAUDE_MD_LINES)).is_empty());
    let over = file("src/x/CLAUDE.md", &blank(MAX_CLAUDE_MD_LINES + 1));
    assert_eq!(rules(&over), [CLAUDE_MD]);
    assert_eq!(over[0].line, MAX_CLAUDE_MD_LINES + 1);
    assert!(over[0].message.starts_with("301/300 lines"));
    assert!(over[0]
        .message
        .contains("move a procedure into the skill or doc"));
}

#[test]
fn a_skill_file_at_its_cap_passes_and_one_line_over_fails() {
    let at = format!("{SKILL_MD_OK}{}", blank(MAX_SKILL_FILE_LINES - 4));
    assert!(file(".claude/skills/s/SKILL.md", &at).is_empty());
    let over = file(
        ".claude/skills/s/brief.md",
        &blank(MAX_SKILL_FILE_LINES + 1),
    );
    assert_eq!(rules(&over), [SKILL]);
    assert!(file(".claude/skills/s/brief.md", &blank(MAX_SKILL_FILE_LINES)).is_empty());
}

#[test]
fn a_cap_is_excused_by_an_allow_with_a_reason_anywhere_in_the_file() {
    let allow = "<!-- lint: allow(claude-md) a generated table that must stay whole -->\n";
    assert!(file("CLAUDE.md", &format!("{}{allow}", blank(400))).is_empty());
    let bare = format!("<!-- lint: {}allow(claude-md) -->\n", "");
    let found = file("CLAUDE.md", &format!("{}{bare}", blank(400)));
    assert_eq!(rules(&found), [CLAUDE_MD, "allow"]);
}

#[test]
fn history_is_flagged_unless_its_line_carries_an_allow() {
    assert_eq!(rules(&file("CLAUDE.md", "see #12\n")), [HISTORY]);
    let allowed = "<!-- lint: allow(history) the number is the example -->\nsee #12\n";
    assert!(file("CLAUDE.md", allowed).is_empty());
    assert!(file("CLAUDE.md", "see #N\n").is_empty());
}

#[test]
fn only_a_skill_md_is_held_to_the_frontmatter() {
    assert_eq!(
        rules(&file(".claude/skills/s/SKILL.md", "# no frontmatter\n")),
        [SKILL]
    );
    assert!(file(".claude/skills/s/brief.md", "# no frontmatter\n").is_empty());
    assert!(file("CLAUDE.md", "# no frontmatter\n").is_empty());
}

#[test]
fn agent_docs_are_claude_md_files_and_skill_folders_files() {
    for p in [
        "CLAUDE.md",
        "src/render/CLAUDE.md",
        ".claude/skills/a/SKILL.md",
        ".claude/skills/a/x/y.md",
    ] {
        assert!(is_agent_doc(p), "{p}");
    }
    for p in [
        "README.md",
        "docs/CLAUDE.md.bak",
        "XCLAUDE.md",
        ".claude/skills/loose.md",
        ".claude/x.md",
    ] {
        assert!(!is_agent_doc(p), "{p}");
    }
}

#[test]
fn the_tree_reads_tracked_files_only_and_flags_a_skill_folder_without_skill_md() {
    let repo = Repo::new("agent-docs-tree", true);
    repo.file("CLAUDE.md", "ok\n");
    repo.file(".claude/skills/s/SKILL.md", SKILL_MD_OK);
    repo.file(".claude/skills/orphan/notes.md", "notes\n");
    repo.file(".claude/agents/coder.md", &agent_md("coder"));
    repo.file(".claude/agents/orchestrator.md", &agent_md("orchestrator"));
    repo.file(".claude/agents/reviewer.md", &agent_md("reviewer"));
    repo.file(".claude/agents/shepherd.md", &agent_md("shepherd"));
    repo.git(&["add", "."]);
    // Untracked: never read, however wrong.
    repo.file("scratch/CLAUDE.md", &blank(MAX_CLAUDE_MD_LINES + 1));
    repo.file(".claude/skills/new/SKILL.md", "no frontmatter, see #3\n");
    let found = tree(&repo.0).expect("git lists the tree");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, ".claude/skills/orphan/");
    assert_eq!(found[0].rule, SKILL);
}

#[test]
fn the_commit_hook_list_checks_only_its_agent_docs() {
    let repo = Repo::new("agent-docs-files", false);
    repo.file("CLAUDE.md", "see #5\n");
    repo.file("README.md", "see #5\n");
    let staged = ["./CLAUDE.md".to_string(), "README.md".to_string()];
    let found = files(&repo.0, &staged);
    assert_eq!(rules(&found), [HISTORY]);
    assert_eq!(found[0].path, "CLAUDE.md");
}

#[test]
fn a_definition_is_held_to_its_cap_its_frontmatter_and_history() {
    let ok = agent_md("helper");
    assert!(file(".claude/agents/helper.md", &ok).is_empty());
    let over = format!("{ok}{}", blank(MAX_AGENT_LINES));
    assert_eq!(rules(&file(".claude/agents/helper.md", &over)), [AGENT]);
    let told = format!("{ok}see #12\n");
    assert_eq!(rules(&file(".claude/agents/helper.md", &told)), [HISTORY]);
    assert!(is_agent_doc(".claude/agents/helper.md"));
    assert!(!is_agent_doc(".claude/agents/x/helper.md"));
    assert!(!is_agent_doc(".claude/agents/helper.txt"));
}

#[test]
fn the_tree_flags_a_pinned_definition_it_does_not_track() {
    let repo = Repo::new("agent-docs-pins", true);
    repo.file(".claude/agents/coder.md", &agent_md("coder"));
    repo.file(".claude/agents/orchestrator.md", &agent_md("orchestrator"));
    repo.file(".claude/agents/shepherd.md", &agent_md("shepherd"));
    repo.git(&["add", "."]);
    let found = tree(&repo.0).expect("git lists the tree");
    assert_eq!(rules(&found), [AGENT], "{found:?}");
    assert_eq!(found[0].path, ".claude/agents/reviewer.md");
}

#[test]
fn a_skill_path_needs_its_folder_and_a_file() {
    assert!(is_agent_doc(".claude/skills/s/SKILL.md"));
    assert!(!is_agent_doc(".claude/skills//SKILL.md"));
    assert!(!is_agent_doc(".claude/skills/s/"));
}

#[test]
fn a_definition_problem_is_reported_through_the_file_pass() {
    let found = file(".claude/agents/helper.md", "---\nname: other\n---\n");
    assert!(rules(&found).contains(&AGENT), "{found:?}");
}
