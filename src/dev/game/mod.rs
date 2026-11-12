//! src/dev/game/ — test games: a Lua recipe builds a small game project headless (#2029).
//!
//! A test game is a folder under the checkout's `games/`:
//!
//! ```text
//! games/<name>/
//!   recipe.lua    builds the project in edit mode, on the one API surface
//!   scripts/      the game's behaviours, copied into assets/scripts/
//!   scenarios/    headless play-tests, in `play`'s scenario format
//!   shots.lua     the fixed cameras the screenshots are taken from
//! ```
//!
//! [`generate`] turns one into a game project (by default under the worktree's
//! gitignored `target/games/<name>/`): it copies the folder's files in, opens the
//! project for editing as `session` does, and runs `recipe.lua` there as one
//! console command, with the command line's [`Params`] as its argument. The
//! recipe writes everything else (scenes, materials, textures, sounds) through the
//! API, so what it cannot do is an API gap, never a generator feature.
//! [`check`] generates it, generates it again to prove the files are the same,
//! runs every scenario as `play` does, replays the demo each recorded and takes
//! the shots, failing on a scenario that fails, a demo that diverges on replay
//! and a shot that is all one colour. The `game` binary is the
//! command (`make game`, `make game-test`), and `tests/gpu/test_games/mod.rs` runs
//! [`check`] on every folder.

mod check;
mod generate;
mod params;
mod tree;

use std::path::{Path, PathBuf};

pub use check::{check, Report};
pub use generate::generate;
pub use params::Params;

/// The file that marks a game folder, and the recipe the generator runs.
pub const RECIPE: &str = "recipe.lua";
/// The file a game's shots are read from, beside the recipe.
pub const SHOTS: &str = "shots.lua";

/// One test game: its name and its folder under `games/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub name: String,
    pub dir: PathBuf,
}

/// The checkout's `games/` folder, which every clone and worktree carries.
pub fn games_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("games")
}

/// Where a game is generated when no `OUT` is named: the worktree's own
/// gitignored `target/games/<name>/`, so two worktrees never share one.
pub fn default_out(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("games")
        .join(name)
}

/// The game called `name` under `root` (normally [`games_dir`]).
pub fn find(root: &Path, name: &str) -> Result<Game, String> {
    let games = all(root)?;
    let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
    let missing = format!("no test game '{name}' (games: {})", names.join(", "));
    games.into_iter().find(|g| g.name == name).ok_or(missing)
}

/// Every folder under `root` (normally [`games_dir`]) holding a [`RECIPE`], by name.
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
