use super::scan::Scan;
use super::wiring::case_for;
use super::{Allow, RULES};
use crate::repo::scaffold::Repo;

fn kept(repo: &Repo, name: &str, path: &str) -> usize {
    let all = super::all(&Scan::at(&repo.0));
    all.iter()
        .filter(|f| f.path == path && f.rule == name)
        .count()
}

const UNEXCUSABLE: &[&str] = &["fold-names", "ignored", "orphan-seeds", "root-tests"];

#[test]
fn an_inline_allow_on_each_row_s_finding_is_honoured_by_its_mode() {
    let mut broken = Vec::new();
    for (name, rule, allow) in RULES {
        let Some(case) = case_for(name) else { continue };
        let repo = Repo::new(&format!("wiring-allow-{name}"), true);
        for (path, body) in &case.files {
            repo.file(path, body);
        }
        repo.git(&["add", "-A"]);
        let Some(hit) = rule(&Scan::at(&repo.0))
            .into_iter()
            .find(|f| f.path.ends_with(case.flags) && f.rule == *name)
        else {
            continue;
        };
        let before = kept(&repo, name, &hit.path);
        let text = std::fs::read_to_string(repo.0.join(&hit.path)).unwrap_or_default();
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        let at = (hit.line.max(1) - 1).min(lines.len());
        lines.insert(at, format!("// lint: allow({name}) wiring"));
        repo.file(&hit.path, &(lines.join("\n") + "\n"));
        let after = kept(&repo, name, &hit.path);
        let excused = after < before;
        if excused == UNEXCUSABLE.contains(name) || excused == (*allow == Allow::Never) {
            broken.push(format!(
                "`{name}`: an inline allow above it: {before} kept, then {after}"
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn an_own_row_s_report_on_its_own_allow_is_not_excused_by_it() {
    let repo = Repo::new("wiring-own", true);
    let op = "crates/edit/src/authoring/light.rs";
    repo.file(
        op,
        "pub fn set_range(c: &mut C) {} // lint: allow(api-parity) why\n",
    );
    repo.git(&["add", "-A"]);
    assert_eq!(kept(&repo, "api-parity", op), 1);
}
