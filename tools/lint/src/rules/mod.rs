//! The rules gate (#934): every check that reports a [`Finding`] and honours the
//! shared inline allow (`finding.rs`). A new rule is a module with its own
//! `<rule>_tests.rs`, plus one line in the sorted `mod` list, one row in the
//! sorted [`RULES`] (#911) and its must-flag case in `wiring.rs`: not a new flag,
//! gate or baseline.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --rules`
//! Exit code 1 on any finding; report mirrored to `.lint/report.txt` (`gates.rs`).

mod finding;
mod scan;
mod test_code;

// sorted: one module per rule, each a row of `RULES`
mod active_self;
mod api_parity;
mod asset_write;
mod atomic_write;
mod binary_init;
mod body_table;
mod cfg_test_switch;
mod converter_log;
mod dir_size;
mod drag_range;
mod ecs_write;
mod engine_spawn;
mod entity_arg;
mod fold_names;
mod full_literal;
mod fullscreen_triangle;
mod gpu_default_only;
mod gpu_ops;
mod held_warn;
mod ignored;
mod key_guard;
mod log_tag;
mod markers;
mod named_enum;
mod one_logger;
mod orphan_module;
mod orphan_seeds;
mod physics_borrow;
mod platform_cfg;
mod prefix;
mod read_keys;
mod root_tests;
mod sanitize;
mod sanitize_rules;
mod self_path;
mod skip;
mod sorted;
mod static_write;
mod stored_format;
mod subprocess;
mod temp_dir;
mod test_shape;
mod tool_named_test;
mod unwrap;
mod world_space;

use crate::gates::Verdict;
pub use finding::{allowed, allowed_in, unexplained, Finding};
use scan::Scan;
use std::collections::BTreeMap;
use std::path::Path;

/// Whether an inline `lint: allow(<rule>) <why>` may excuse a rule's findings.
#[derive(PartialEq)]
enum Allow {
    Inline,
    /// Nothing may excuse it: `ignored` and `orphan-seeds`, whose fixes are a
    /// `.gitignore` pattern and a moved file, `key-guard`, whose finding sits on a
    /// manifest, where no inline allow can go, `root-tests`, whose fix is the
    /// manifest key or a moved test, and `fold-names`, which an allow would not
    /// make fold.
    Never,
    /// The rule reads its own allows, so it can report a stale one
    /// (`api-parity`, whose allow sits on the op, not on the finding's cause;
    /// `dir-size`, whose allow sits anywhere in the directory's owner file), or so
    /// one of its findings stays unexcusable (`orphan-module`, whose unreadable
    /// workspace sits on a manifest).
    Own,
}

/// A rule: every finding it has over the scan.
type Rule = fn(&Scan) -> Vec<Finding>;

/// Every rule, by the name its findings carry (a rule reporting several names,
/// `test-shape`'s, goes by one). A new rule is one row, in its sorted place, and a
/// must-flag case in `wiring.rs`, whose test fails naming any row without one.
const RULES: &[(&str, Rule, Allow)] = &[
    // sorted: rules are independent; allows apply to each rule's own findings
    (active_self::RULE, active_self::run, Allow::Inline),
    (api_parity::RULE, api_parity::run, Allow::Own),
    (asset_write::RULE, asset_write::run, Allow::Inline),
    (atomic_write::RULE, atomic_write::run, Allow::Inline),
    (binary_init::RULE, binary_init::run, Allow::Inline),
    (body_table::RULE, body_table::run, Allow::Inline),
    (cfg_test_switch::RULE, cfg_test_switch::run, Allow::Inline),
    (converter_log::RULE, converter_log::run, Allow::Inline),
    (dir_size::RULE, dir_size::run, Allow::Own),
    (drag_range::RULE, drag_range::run, Allow::Inline),
    (ecs_write::RULE, ecs_write::run, Allow::Inline),
    (engine_spawn::RULE, engine_spawn::run, Allow::Inline),
    (entity_arg::RULE, entity_arg::run, Allow::Inline),
    (fold_names::RULE, fold_names::run, Allow::Never),
    (full_literal::RULE, full_literal::run, Allow::Inline),
    (
        fullscreen_triangle::RULE,
        fullscreen_triangle::run,
        Allow::Inline,
    ),
    (gpu_default_only::RULE, gpu_default_only::run, Allow::Inline),
    (gpu_ops::RULE, gpu_ops::run, Allow::Inline),
    (held_warn::RULE, held_warn::run, Allow::Inline),
    (ignored::RULE, ignored::run, Allow::Never),
    (key_guard::RULE, key_guard::run, Allow::Never),
    (log_tag::RULE, log_tag::run, Allow::Inline),
    (markers::RULE, markers::run, Allow::Inline),
    (named_enum::RULE, named_enum::run, Allow::Inline),
    (one_logger::RULE, one_logger::run, Allow::Inline),
    (orphan_module::RULE, orphan_module::run, Allow::Own),
    (orphan_seeds::RULE, orphan_seeds::run, Allow::Never),
    (physics_borrow::RULE, physics_borrow::run, Allow::Inline),
    (platform_cfg::RULE, platform_cfg::run, Allow::Inline),
    (prefix::RULE, prefix::run, Allow::Inline),
    (read_keys::RULE, read_keys::run, Allow::Inline),
    (root_tests::RULE, root_tests::run, Allow::Never),
    (sanitize::RULE, sanitize::run, Allow::Inline),
    (self_path::RULE, self_path::run, Allow::Inline),
    (skip::RULE, skip::run, Allow::Inline),
    (sorted::RULE, sorted::run, Allow::Inline),
    (static_write::RULE, static_write::run, Allow::Inline),
    (stored_format::RULE, stored_format::run, Allow::Inline),
    (subprocess::RULE, subprocess::run, Allow::Inline),
    (temp_dir::RULE, temp_dir::run, Allow::Inline),
    (test_shape::INNER_CFG, test_shape::run, Allow::Inline),
    (tool_named_test::RULE, tool_named_test::run, Allow::Inline),
    (unwrap::RULE, unwrap::run, Allow::Inline),
    (world_space::RULE, world_space::run, Allow::Inline),
];

/// The gate over the repository at `base`: every rule over
/// [`crate::size::SCAN_ROOTS`], each allow it honours applied.
pub fn verdict(base: &Path, _: &[String]) -> Verdict {
    let scan = Scan::at(base);
    let findings = all(&scan);
    let body = render(&findings, &crowded(&scan.dirs()));
    Verdict {
        findings: findings.iter().map(ToString::to_string).collect(),
        body,
        advice: String::new(),
    }
}

/// Every rule's findings over `scan`, minus what an inline allow excuses, plus
/// every allow without a reason.
fn all(scan: &Scan) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (_, rule, allow) in RULES {
        let mut found = rule(scan);
        if *allow == Allow::Inline {
            found.retain(|f| !finding::allowed(scan.source(&f.path), f.line, f.rule));
        }
        findings.extend(found);
    }
    findings.extend(scan.each_file(finding::unexplained));
    findings
}

/// Directories past the ≲10-files aim, most crowded first, counted as `dir-size`
/// counts (#1367). A signal, never a failure: what is past the gate's cap under
/// `src/` is that rule's finding or allow, so it is left out here.
fn crowded<'a>(dirs: &BTreeMap<&'a str, Vec<&str>>) -> Vec<(&'a str, usize)> {
    let mut over: Vec<(&str, usize)> = dirs
        .iter()
        .map(|(d, names)| (*d, dir_size::sources(d, names).len()))
        .filter(|(_, n)| *n > prefix::AIM_FILES_PER_DIR)
        .filter(|(d, n)| {
            let gated = crate::modules::module_path(d).starts_with("src/");
            !(gated && *n > dir_size::MAX_FILES_PER_DIR)
        })
        .collect();
    over.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    over
}

fn render(findings: &[Finding], crowded: &[(&str, usize)]) -> String {
    let mut body = String::new();
    for f in findings {
        body.push_str(&format!("{f}\n"));
    }
    if !crowded.is_empty() {
        body.push_str(&format!(
            "signal (not a failure): {} directories hold more than {} source files; \
             cluster them by responsibility when you next work there:\n",
            crowded.len(),
            prefix::AIM_FILES_PER_DIR
        ));
        for (d, n) in crowded {
            body.push_str(&format!("  {n:>3}  {d}/\n"));
        }
    }
    body.push_str(&match findings.len() {
        0 => "rules: ok\n".to_string(),
        n => format!("rules: FAILED, {n} finding(s)\n"),
    });
    body
}

#[cfg(test)]
mod lockfile_tests;
#[cfg(test)]
mod wiring;
#[cfg(test)]
mod wiring_allows;
