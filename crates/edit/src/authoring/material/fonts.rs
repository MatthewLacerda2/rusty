use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusty_core::project::ContentRoots;
use rusty_procgen::{FontHandle, Sources, TextureRecipe};

/// Bytes: far past any real face, short of reading a misnamed video into memory.
const MAX_FONT_BYTES: u64 = 64 << 20;

pub(super) fn read_fonts(
    recipe: &TextureRecipe,
    roots: &ContentRoots,
    sources: &mut Sources,
) -> Result<(), String> {
    for (node, path) in recipe.font_paths() {
        let fault = |why: &str| format!("text node '{node}' can't read font {path:?}: {why}");
        let file = roots.file(path);
        let stamp = match std::fs::metadata(&file) {
            Ok(m) if m.is_file() => Stamp {
                len: m.len(),
                modified: m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()),
            },
            _ => return Err(fault("no such file")),
        };
        let len = stamp.len;
        if len > MAX_FONT_BYTES {
            return Err(fault(&format!(
                "{len} bytes is past the {MAX_FONT_BYTES} a font may be"
            )));
        }
        super::sources::warn_if_outside(roots, "font", path);
        let mut kept = parsed().lock().unwrap_or_else(|e| e.into_inner());
        match kept.get(&file) {
            Some((at, font)) if *at == stamp && stamp.modified.is_some() => {
                sources.insert_parsed_font(path, font.clone());
            }
            _ => {
                let bytes = std::fs::read(&file).map_err(|e| fault(&e.to_string()))?;
                let font = sources.insert_font(path, bytes).map_err(|e| fault(&e))?;
                kept.insert(file, (stamp, font));
            }
        }
    }
    Ok(())
}

/// Compared for equality, never against a clock; an unreadable modified time is never cached.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<Duration>,
}

fn parsed() -> &'static Mutex<BTreeMap<PathBuf, (Stamp, FontHandle)>> {
    static PARSED: OnceLock<Mutex<BTreeMap<PathBuf, (Stamp, FontHandle)>>> = OnceLock::new();
    PARSED.get_or_init(Mutex::default)
}

#[cfg(test)]
mod tests {
    use super::super::load_recipe;
    use rusty_core::project::ContentRoots;
    use rusty_procgen::TextureRecipe;

    fn text_recipe(font: &str) -> TextureRecipe {
        let node = serde_json::json!({ "id": "sign", "op": "text", "text": "EXIT", "font": font });
        TextureRecipe::new(8, vec![serde_json::from_value(node).unwrap()])
    }

    fn default_bytes() -> Vec<u8> {
        let engine =
            rusty_core::project::checkout().join("engine/fonts/InstrumentSans-Regular.ttf");
        std::fs::read(engine).unwrap()
    }

    #[test]
    fn a_font_is_read_from_the_root_and_bakes() {
        let root = rusty_test_support::temp::dir().join("rusty_fonts_root_2190");
        std::fs::create_dir_all(root.join("fonts")).unwrap();
        std::fs::write(root.join("fonts").join("sign.ttf"), default_bytes()).unwrap();
        let loaded =
            load_recipe(&text_recipe("fonts/sign.ttf"), &ContentRoots::new(&root)).unwrap();
        let font = loaded.sources.font("fonts/sign.ttf").unwrap();
        rusty_procgen::evaluate_with(&loaded.recipe, &loaded.sources).unwrap();
        let again = load_recipe(&text_recipe("fonts/sign.ttf"), &ContentRoots::new(&root)).unwrap();
        let kept = again.sources.font("fonts/sign.ttf").unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&font, &kept),
            "a re-load keeps the parsed font"
        );
        let engine = text_recipe("engine/fonts/InstrumentSans-Regular.ttf");
        let loaded = load_recipe(&engine, &ContentRoots::new(&root)).unwrap();
        assert!(loaded
            .sources
            .font("engine/fonts/InstrumentSans-Regular.ttf")
            .is_some());
        let empty = rusty_test_support::temp::dir().join("rusty_fonts_none_2190");
        assert!(load_recipe(&text_recipe(""), &ContentRoots::new(&empty)).is_ok());
    }

    #[test]
    fn a_missing_or_truncated_font_is_an_error_naming_the_node_and_the_path() {
        let root = rusty_test_support::temp::dir().join("rusty_fonts_bad_2190");
        std::fs::create_dir_all(&root).unwrap();
        let e = load_recipe(&text_recipe("nope.ttf"), &ContentRoots::new(&root)).unwrap_err();
        assert!(
            e.contains("text node 'sign'") && e.contains(r#""nope.ttf": no such file"#),
            "{e}"
        );
        let bytes = default_bytes();
        std::fs::write(root.join("cut.ttf"), &bytes[..bytes.len() / 50]).unwrap();
        std::fs::write(root.join("junk.ttf"), b"not a font").unwrap();
        for bad in ["cut.ttf", "junk.ttf"] {
            let e = load_recipe(&text_recipe(bad), &ContentRoots::new(&root)).unwrap_err();
            assert!(
                e.contains("text node 'sign'") && e.contains(bad) && e.contains("font"),
                "{e}"
            );
        }
    }
}
