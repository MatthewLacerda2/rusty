//! src/asset/lightmap_page/ — a baked lightmap page as an asset (#1694), Unity's
//! *Lightmap* texture type.
//!
//! A lightmap bake writes its pages into one folder beside its scene,
//! `<scene file>`[`DIR_SUFFIX`] (`dev::bake::lightmap::lightmap_dir`). Those pages
//! are not colour textures: the generic texture importer (`texture_import`) leaves
//! every file under such a folder alone ([`is_baked`]), so a page never gets a BC7
//! sRGB cache nothing reads, and the renderer loads the page the bake wrote.

use std::path::Path;

/// The suffix of the folder beside a scene file that holds its baked lightmap
/// pages: `foo.scene` → `foo.scene.lightmaps/`.
pub const DIR_SUFFIX: &str = ".lightmaps";

/// Whether `path` lies in a scene's lightmap folder (a folder whose name ends in
/// [`DIR_SUFFIX`]): a baked page the texture importer leaves alone. The one check
/// every caller asks.
pub fn is_baked(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    parent.components().any(|c| match c {
        std::path::Component::Normal(name) => name
            .to_str()
            .is_some_and(|n| n.len() > DIR_SUFFIX.len() && n.ends_with(DIR_SUFFIX)),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_under_a_scene_lightmap_folder_are_baked_and_nothing_else_is() {
        let baked = [
            "assets/scenes/kolvik.scene.lightmaps/lightmap_0_ab12cd34.png",
            "assets/a.scene.lightmaps/deeper/page.png",
        ];
        for path in baked {
            assert!(is_baked(Path::new(path)), "{path}");
        }
        let not = [
            "assets/textures/floor.png",
            "assets/scenes/kolvik.scene.lightmaps",
            "assets/.lightmaps/page.png",
            "assets/lightmaps/page.png",
            "assets/my.lightmaps.png",
        ];
        for path in not {
            assert!(!is_baked(Path::new(path)), "{path}");
        }
    }
}
