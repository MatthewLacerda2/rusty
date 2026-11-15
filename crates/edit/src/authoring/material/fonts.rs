//! crates/edit/src/authoring/material/fonts.rs — read the font files a recipe's
//! `text` nodes name (#2190), as [`super::sources`] reads its images.
//!
//! procgen reads no files, so each path is taken from the project root, read and
//! parsed here before anything bakes: a file that isn't there, is larger than a
//! font has any need to be, or doesn't parse is an error naming the node and the
//! path. A `text` node on the engine's default font (`font = ""`) names no file:
//! the default is compiled into the engine, so a shipped game bakes it with no
//! file beside it. Fonts are small and read whole, not lazily as images are.

use std::path::Path;

use rusty_procgen::{Sources, TextureRecipe};

/// Largest font file read, bytes: far past any real face, short of reading a
/// misnamed video into memory.
const MAX_FONT_BYTES: u64 = 64 << 20;

/// Read and parse every font `recipe`'s `text` nodes name, each from `root`, into
/// `sources`.
pub(super) fn read_fonts(
    recipe: &TextureRecipe,
    root: &Path,
    sources: &mut Sources,
) -> Result<(), String> {
    for (node, path) in recipe.font_paths() {
        let fault = |why: &str| format!("text node '{node}' can't read font {path:?}: {why}");
        let file = root.join(path);
        let len = match std::fs::metadata(&file) {
            Ok(m) if m.is_file() => m.len(),
            _ => return Err(fault("no such file")),
        };
        if len > MAX_FONT_BYTES {
            return Err(fault(&format!(
                "{len} bytes is past the {MAX_FONT_BYTES} a font may be"
            )));
        }
        super::sources::warn_if_outside(&file, root, "font", path);
        let bytes = std::fs::read(&file).map_err(|e| fault(&e.to_string()))?;
        sources.insert_font(path, bytes).map_err(|e| fault(&e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::load_recipe;
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
        let loaded = load_recipe(&text_recipe("fonts/sign.ttf"), &root).unwrap();
        assert!(loaded.sources.font("fonts/sign.ttf").is_some());
        rusty_procgen::evaluate_with(&loaded.recipe, &loaded.sources).unwrap();
        // The default font needs no file at all.
        let empty = rusty_test_support::temp::dir().join("rusty_fonts_none_2190");
        assert!(load_recipe(&text_recipe(""), &empty).is_ok());
    }

    #[test]
    fn a_missing_or_truncated_font_is_an_error_naming_the_node_and_the_path() {
        let root = rusty_test_support::temp::dir().join("rusty_fonts_bad_2190");
        std::fs::create_dir_all(&root).unwrap();
        let e = load_recipe(&text_recipe("nope.ttf"), &root).unwrap_err();
        assert!(
            e.contains("text node 'sign'") && e.contains(r#""nope.ttf": no such file"#),
            "{e}"
        );
        let bytes = default_bytes();
        std::fs::write(root.join("cut.ttf"), &bytes[..bytes.len() / 50]).unwrap();
        std::fs::write(root.join("junk.ttf"), b"not a font").unwrap();
        for bad in ["cut.ttf", "junk.ttf"] {
            let e = load_recipe(&text_recipe(bad), &root).unwrap_err();
            assert!(
                e.contains("text node 'sign'") && e.contains(bad) && e.contains("font"),
                "{e}"
            );
        }
    }
}
