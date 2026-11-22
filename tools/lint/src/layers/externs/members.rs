use super::cargo::{dev_keys, keys};
use super::policy::Policy;
use crate::layers::table::Layer;

pub fn stray(module: &str, text: &str, layers: &[Layer], policy: &Policy) -> Vec<String> {
    let Some(row) = layers.iter().find(|l| l.module == module) else {
        return Vec::new();
    };
    let declared = keys(text);
    let mut out = Vec::new();
    for key in &declared {
        let ok = match key.strip_prefix("rusty_") {
            Some(dep) => row.deps.contains(&dep),
            None => row.crates.contains(&key.as_str()) || policy.common.contains(&key.as_str()),
        };
        if !ok {
            let name = key.replace('_', "-");
            out.push(format!(
                "STRAY_CRATE crates/{module}/Cargo.toml → {name} — not in {module}'s row; \
                 drop it, or add it to the row if {module}'s code names it"
            ));
        }
    }
    let wanted = (row.deps.iter().map(|d| format!("rusty_{d}")))
        .chain(row.crates.iter().map(|k| k.to_string()));
    for key in wanted.filter(|k| !declared.contains(k)) {
        let name = key.replace('_', "-");
        out.push(format!(
            "MISSING_CRATE crates/{module}/Cargo.toml → {name} — {module}'s row names it but \
             its manifest does not depend on it; add the dependency or drop it from the row"
        ));
    }
    out
}

/// Cargo accepts a dev-dependency cycle; the table does not.
pub fn dev_stray(module: &str, text: &str, layers: &[Layer]) -> Vec<String> {
    let Some(at) = layers.iter().position(|l| l.module == module) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in dev_keys(text) {
        let Some(dep) = key.strip_prefix("rusty_") else {
            continue;
        };
        let Some(row) = layers
            .iter()
            .position(|l| l.module.replace('-', "_") == dep)
        else {
            continue;
        };
        let dep = layers[row].module;
        if row < at {
            continue;
        }
        out.push(format!(
            "STRAY_DEV_CRATE crates/{module}/Cargo.toml → rusty-{dep} — a dev-dependency on \
             a module that is not below {module} in the layer table makes a cycle; move the \
             test to `tests/` or to {dep}'s own tests"
        ));
    }
    out
}

pub fn display(module: &str, text: &str, layers: &[Layer], policy: &Policy) -> Vec<String> {
    let Some(at) = layers.iter().position(|l| l.module == module && l.sim) else {
        return Vec::new();
    };
    let mut out: Vec<String> = all_keys(text)
        .iter()
        .filter(|k| policy.is_display(k))
        .map(|k| {
            format!(
                "WRONG_DIRECTION crates/{module}/Cargo.toml → {} — a sim crate never depends \
                 on the display layer, not even in tests; move the type to the sim side \
                 (components / scene / core) and let the display crate import it",
                k.replace('_', "-")
            )
        })
        .collect();
    for key in dev_keys(text) {
        let below = layers[..at].iter().find(|l| {
            !l.sim && Some(l.module.replace('-', "_").as_str()) == key.strip_prefix("rusty_")
        });
        if let Some(l) = below {
            out.push(format!(
                "PLATFORM_DEV_CRATE crates/{module}/Cargo.toml → rusty-{} — a sim crate's tests \
                 take only sim siblings; test the pair from `tests/` (the root's integration tests)",
                l.module
            ));
        }
    }
    out
}

pub fn support(module: &str, text: &str) -> Vec<String> {
    if !crate::modules::NOT_MODULES
        .iter()
        .any(|(m, _)| *m == module)
    {
        return Vec::new();
    }
    all_keys(text)
        .iter()
        .filter(|k| k.starts_with("rusty_") || super::policy::POLICY.is_display(k))
        .map(|k| {
            format!(
                "SUPPORT_REACH crates/{module}/Cargo.toml → {} — every member may take {module} \
                 as a dev-dependency, so it names no workspace crate and no display crate; \
                 move the helper into the crate that needs it",
                k.replace('_', "-")
            )
        })
        .collect()
}

fn all_keys(text: &str) -> Vec<String> {
    let mut named = keys(text);
    named.extend(dev_keys(text));
    named.sort();
    named.dedup();
    named
}

#[cfg(test)]
#[path = "members_tests.rs"]
mod tests;
