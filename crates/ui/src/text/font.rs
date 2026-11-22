//! crates/ui/src/text/font.rs — the UI's fonts by path.

use rusty_core::collections::Map;
use std::sync::{Arc, Mutex, OnceLock};

use rusty_font::{default_font, FontData, FontHandle};

type Cache = Mutex<Map<Option<String>, FontHandle>>;

/// Process-wide by path, so the sim's layout and the renderer's atlas (`render::ui::text`)
/// share one parsed font: the same glyph ids.
fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Map::default()))
}

/// The font at `path` (`None`: the bundled default), loading it on first use. A
/// path that fails to load resolves to the default font.
pub fn load(path: Option<&str>) -> FontHandle {
    let key = path.map(str::to_string);
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(f) = cache.get(&key) {
        return Arc::clone(f);
    }
    let parsed = path.and_then(|p| FontData::parse(std::fs::read(p).ok()?.into()));
    let font = match parsed {
        Some(f) => Arc::new(f),
        None => {
            if let Some(p) = path {
                log::warn!("[UI] font '{p}' did not load; using the default font");
            }
            default_font()
        }
    };
    cache.insert(key, Arc::clone(&font));
    font
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_path_falls_back_to_the_default_font() {
        let bad = load(Some("no/such/font.ttf"));
        assert!(
            Arc::ptr_eq(&default_font(), &bad),
            "a missing font resolves to the default"
        );
        assert!(Arc::ptr_eq(&load(None), &default_font()));
    }
}
