mod check;
mod generate;
mod params;
mod tree;

use std::path::{Path, PathBuf};

pub use check::{check, Report};
pub use generate::generate;
pub use params::Params;

pub const RECIPE: &str = "recipe.lua";
pub const SHOTS: &str = "shots.lua";
/// The folder beside the games holding what several share (a HUD), copied into each
/// generated game's `assets/scripts/shared/`, so a game attaches it by a path inside
/// its own project (`Scene.AddScript(id, "assets/scripts/shared/hud.lua")`).
pub const SHARED: &str = "shared";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub name: String,
    pub dir: PathBuf,
}

pub fn games_dir() -> PathBuf {
    rusty_core::project::checkout().join("games")
}

pub fn default_out(name: &str) -> PathBuf {
    rusty_core::project::checkout()
        .join("target")
        .join("games")
        .join(name)
}

pub fn find(root: &Path, name: &str) -> Result<Game, String> {
    let games = all(root)?;
    let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
    let missing = format!("no test game '{name}' (games: {})", names.join(", "));
    games.into_iter().find(|g| g.name == name).ok_or(missing)
}

pub fn all(root: &Path) -> Result<Vec<Game>, String> {
    let entries = std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let mut games: Vec<Game> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.join(RECIPE).is_file())
        .filter_map(|dir| {
            let name = dir.file_name()?.to_str()?.to_string();
            Some(Game { name, dir })
        })
        .collect();
    games.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(games)
}
