mod finding;
mod scan;
mod test_code;

// sorted: one module per rule, each a row of `RULES`
mod active_self;
mod api_parity;
mod asset_write;
mod atomic_write;
mod binary_init;
mod bind_group_index;
mod body_table;
mod cfg_test_switch;
mod converter_log;
mod dir_size;
mod drag_range;
mod drawn_mode;
mod dropped_result;
mod ecs_write;
mod entity_arg;
mod fold_names;
mod full_literal;
mod fullscreen_triangle;
mod gpu_default_only;
mod gpu_ops;
mod held_warn;
mod hook_line;
mod ignored;
mod log_tag;
mod markers;
mod mesh_buffer_write;
mod named_enum;
mod one_logger;
mod orphan_module;
mod orphan_seeds;
mod physics_borrow;
mod platform_cfg;
mod prefix;
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
mod ui_coverage;
mod unwrap;
mod world_space;

use crate::gates::Verdict;
pub use finding::{allowed, allowed_anywhere, allowed_in, unexplained, Finding};
use scan::Scan;
use std::collections::BTreeMap;
use std::path::Path;

/// Whether an inline `lint: allow(<rule>) <why>` may excuse a rule's findings.
#[derive(PartialEq)]
enum Allow {
    Inline,
    Never,
    Own,
}

type Rule = fn(&Scan) -> Vec<Finding>;

const RULES: &[(&str, Rule, Allow)] = &[
    // sorted: rules are independent; allows apply to each rule's own findings
    (active_self::RULE, active_self::run, Allow::Inline),
    (api_parity::RULE, api_parity::run, Allow::Own),
    (asset_write::RULE, asset_write::run, Allow::Inline),
    (atomic_write::RULE, atomic_write::run, Allow::Inline),
    (binary_init::RULE, binary_init::run, Allow::Inline),
    (bind_group_index::RULE, bind_group_index::run, Allow::Inline),
    (body_table::RULE, body_table::run, Allow::Inline),
    (cfg_test_switch::RULE, cfg_test_switch::run, Allow::Inline),
    (converter_log::RULE, converter_log::run, Allow::Inline),
    (dir_size::RULE, dir_size::run, Allow::Own),
    (drag_range::RULE, drag_range::run, Allow::Inline),
    (drawn_mode::RULE, drawn_mode::run, Allow::Inline),
    (dropped_result::RULE, dropped_result::run, Allow::Inline),
    (ecs_write::RULE, ecs_write::run, Allow::Inline),
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
    (hook_line::RULE, hook_line::run, Allow::Inline),
    (ignored::RULE, ignored::run, Allow::Never),
    (log_tag::RULE, log_tag::run, Allow::Inline),
    (markers::RULE, markers::run, Allow::Inline),
    (
        mesh_buffer_write::RULE,
        mesh_buffer_write::run,
        Allow::Inline,
    ),
    (named_enum::RULE, named_enum::run, Allow::Inline),
    (one_logger::RULE, one_logger::run, Allow::Inline),
    (orphan_module::RULE, orphan_module::run, Allow::Own),
    (orphan_seeds::RULE, orphan_seeds::run, Allow::Never),
    (physics_borrow::RULE, physics_borrow::run, Allow::Inline),
    (platform_cfg::RULE, platform_cfg::run, Allow::Inline),
    (prefix::RULE, prefix::run, Allow::Inline),
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
    (ui_coverage::RULE, ui_coverage::run, Allow::Own),
    (unwrap::RULE, unwrap::run, Allow::Inline),
    (world_space::RULE, world_space::run, Allow::Inline),
];

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
