//! src/scripting/discovery.rs — find MonoBehaviour-equivalent project scripts.
//!
//! A project `.lua` script is a "MonoBehaviour" (addable from the inspector's Add
//! Component menu, #83) when it `return`s a table exposing at least one lifecycle
//! callback (the shared `callbacks::LIFECYCLE_CALLBACKS` list). Plain `require`-d
//! helper modules, which return values/functions but no lifecycle table, are NOT
//! offered.
//!
//! Detection evaluates each chunk in a throwaway sandboxed `Lua` and inspects the
//! returned table — the same eval the runtime does, so the menu can never offer a
//! script the runtime would reject. No wall-clock / RNG, so the determinism guard
//! is satisfied.

use std::path::Path;

use mlua::{Lua, Table, Value};

use super::callbacks::LIFECYCLE_CALLBACKS;

/// Scan `dir` (non-recursively) for `.lua` files that expose a lifecycle table.
/// Returns their paths (using `dir`'s separator), sorted for a stable menu order.
/// Unreadable dirs / files and scripts that fail to compile are simply skipped.
pub fn monobehaviour_scripts(dir: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                return None;
            }
            let code = std::fs::read_to_string(&path).ok()?;
            if exposes_lifecycle(&code) {
                Some(path.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    found.sort_unstable();
    found
}

/// True if `code`, evaluated as a chunk, returns a table with at least one
/// lifecycle method. A fresh `Lua` per call keeps detection side-effect-free.
pub fn exposes_lifecycle(code: &str) -> bool {
    let lua = Lua::new();
    let Ok(table) = lua.load(code).eval::<Table>() else {
        return false;
    };
    LIFECYCLE_CALLBACKS
        .iter()
        .any(|m| matches!(table.get::<_, Value>(*m), Ok(Value::Function(_))))
}

/// Display name (file stem) for a script path, for the Add Component menu label.
pub fn script_label(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_table_is_detected() {
        assert!(exposes_lifecycle("return { Start = function() end }"));
        assert!(exposes_lifecycle(
            "return { Update = function(id, dt) end }"
        ));
        assert!(exposes_lifecycle(
            "local M = {}\nfunction M.OnTrigger(a, b) end\nreturn M"
        ));
    }

    #[test]
    fn helper_module_without_lifecycle_is_rejected() {
        // A plain helper: returns a table of non-lifecycle functions.
        assert!(!exposes_lifecycle("return { helper = function() end }"));
        // Returns a non-table value.
        assert!(!exposes_lifecycle("return 42"));
        // A lifecycle name that is NOT a function does not count.
        assert!(!exposes_lifecycle("return { Start = 7 }"));
        // Fails to compile.
        assert!(!exposes_lifecycle("this is not lua"));
    }

    #[test]
    fn label_is_the_file_stem() {
        assert_eq!(script_label("project/assets/scripts/bot.lua"), "bot");
        assert_eq!(script_label("player_controller.lua"), "player_controller");
    }

    #[test]
    fn nonexistent_dir_returns_empty() {
        assert!(monobehaviour_scripts("/nonexistent/dir/xyz").is_empty());
    }

    #[test]
    fn monobehaviour_scripts_finds_lifecycle_scripts_and_skips_helpers() {
        let dir = "/tmp/rusty_discovery_test";
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(format!("{dir}/a.lua"), "return { Start = function() end }").unwrap();
        std::fs::write(format!("{dir}/b.lua"), "return { helper = function() end }").unwrap();
        std::fs::write(format!("{dir}/c.lua"), "return { Update = function() end }").unwrap();
        let found = monobehaviour_scripts(dir);
        assert_eq!(found.len(), 2, "only lifecycle scripts returned");
        // Match on the file name, not a hardcoded separator: paths come back
        // with `\` on Windows and `/` elsewhere.
        assert!(found.iter().any(|p| p.ends_with("a.lua")));
        assert!(found.iter().any(|p| p.ends_with("c.lua")));
        assert!(!found.iter().any(|p| p.ends_with("b.lua")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn monobehaviour_scripts_results_are_sorted() {
        let dir = "/tmp/rusty_discovery_sorted";
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(format!("{dir}/z.lua"), "return { Update = function() end }").unwrap();
        std::fs::write(format!("{dir}/a.lua"), "return { Start = function() end }").unwrap();
        let found = monobehaviour_scripts(dir);
        assert_eq!(found.len(), 2);
        assert!(found[0] < found[1], "results must be sorted");
        let _ = std::fs::remove_dir_all(dir);
    }
}
