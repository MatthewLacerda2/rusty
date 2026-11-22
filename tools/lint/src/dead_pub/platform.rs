use crate::code::code_lines;
use std::path::Path;

const MARKERS: &[&str] = &[
    "cfg(target_os",
    "cfg(not(target_os",
    "cfg(all(target_os",
    "cfg(any(target_os",
];

pub fn mentions_platform(source: &str) -> bool {
    code_lines(source)
        .iter()
        .any(|l| MARKERS.iter().any(|m| l.contains(m)))
}

pub fn is_gated(tree: &Path, path: &str) -> bool {
    let file = Path::new(path);
    let read = |p: &Path| std::fs::read_to_string(tree.join(p)).unwrap_or_default();
    if mentions_platform(&read(file)) {
        return true;
    }
    let (Some(stem), Some(dir)) = (file.file_stem().and_then(|s| s.to_str()), file.parent()) else {
        return false;
    };
    let (name, dir) = if stem == "mod" {
        let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
        (name, dir.parent().unwrap_or(dir))
    } else {
        (stem, dir)
    };
    let parents = [
        dir.join("mod.rs"),
        dir.with_extension("rs"),
        dir.join("lib.rs"),
    ];
    parents
        .iter()
        .any(|p| declared_under_platform_cfg(&read(p), name))
}

fn declared_under_platform_cfg(source: &str, name: &str) -> bool {
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    lines.iter().enumerate().any(|(i, l)| {
        let decl = l
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub ");
        decl.strip_prefix("mod ")
            .is_some_and(|r| r.trim_end_matches(';').trim() == name)
            && lines[..i]
                .iter()
                .rev()
                .take_while(|a| a.starts_with("#["))
                .any(|a| mentions_platform(a))
    })
}
