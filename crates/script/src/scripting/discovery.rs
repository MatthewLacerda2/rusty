//! Finds the project scripts the Add Component menu offers: those returning a table with
//! at least one lifecycle function, so a plain `require`d helper module is left out.
//!
//! Each chunk runs in `inspection_vm`, without `io` and `os` as in Play. One that fails to
//! load or run is not offered and the console says why once, as Unity leaves a script with
//! compile errors out of *Add Component*.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use mlua::Value;

use super::callbacks::LIFECYCLE_CALLBACKS;

pub fn monobehaviour_scripts(dir: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !rusty_core::paths::has_extension(&path, "lua") {
                return None;
            }
            let code = std::fs::read_to_string(&path).ok()?;
            let path = path.to_string_lossy().into_owned();
            match offered(&code) {
                Ok(true) => Some(path),
                Ok(false) => None,
                Err(why) => {
                    warn_once(&path, &why);
                    None
                }
            }
        })
        .collect();
    found.sort_unstable();
    found
}

pub(crate) fn exposes_lifecycle(code: &str) -> bool {
    offered(code).unwrap_or(false)
}

/// `Err` (the error's first line) when the chunk fails to load or run; `Ok(false)` for a helper.
fn offered(code: &str) -> Result<bool, String> {
    let ran = super::sandbox::inspection_vm().and_then(|lua| lua.load(code).eval::<Value>());
    let table = match ran {
        Ok(Value::Table(table)) => table,
        Ok(_) => return Ok(false),
        Err(e) => return Err(e.to_string().lines().next().unwrap_or_default().to_owned()),
    };
    Ok(LIFECYCLE_CALLBACKS
        .iter()
        .any(|m| matches!(table.get::<Value>(*m), Ok(Value::Function(_)))))
}

// The menu lists scripts every frame it is open; a changed error is said again.
thread_local!(static WARNED: RefCell<BTreeSet<(String, String)>> = const { RefCell::new(BTreeSet::new()) });

fn first_time(path: &str, why: &str) -> bool {
    WARNED.with(|w| w.borrow_mut().insert((path.to_owned(), why.to_owned())))
}

fn warn_once(path: &str, why: &str) {
    if first_time(path, why) {
        log::warn!("[Script] {path} is not offered in Add Component: it fails to load: {why}");
    }
}

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
        assert!(!exposes_lifecycle("return { helper = function() end }"));
        assert!(!exposes_lifecycle("return 42"));
        assert!(!exposes_lifecycle("return { Start = 7 }"));
        assert!(!exposes_lifecycle("this is not lua"));
    }

    #[test]
    fn the_inspection_vm_has_no_os_and_no_io() {
        let lua = crate::scripting::sandbox::inspection_vm().unwrap();
        for lib in ["os", "io"] {
            assert!(lua.globals().get::<Value>(lib).unwrap().is_nil(), "{lib}");
        }
        assert!(!lua.globals().get::<Value>("Transform").unwrap().is_nil());
    }

    #[test]
    fn a_chunk_reaching_for_os_or_io_at_load_is_not_offered_and_says_why() {
        for call in ["os.remove('x')", "io.open('x', 'w')"] {
            let code = format!("{call}\nreturn {{ Start = function() end }}");
            assert!(!exposes_lifecycle(&code), "{call}");
            let why = offered(&code).unwrap_err();
            assert!(why.contains("attempt to index a nil value"), "{why}");
        }
        assert!(exposes_lifecycle(
            "return { Start = function() os.remove('x') end }"
        ));
    }

    #[test]
    fn a_failing_script_is_warned_about_once_per_error() {
        assert!(first_time("bad.lua", "e1"));
        assert!(!first_time("bad.lua", "e1"));
        assert!(first_time("bad.lua", "e2"));
    }

    #[test]
    fn label_is_the_file_stem() {
        assert_eq!(script_label("assets/scripts/bot.lua"), "bot");
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
