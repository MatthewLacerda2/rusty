//! Generate a test game's project: copy its files in, then run its recipe in edit
//! mode, as one `session` command.

use std::path::Path;

use super::{tree, Game, Params, RECIPE, SHOTS};
use crate::app::ProjectSource;
use crate::core::project::{Access, BootPlan};
use crate::dev::session::Session;

/// Build `game` into the project folder `out` (made, or cleared when an earlier
/// generation made it) with the recipe's `params`.
///
/// The game's `scripts/` go to `assets/scripts/`, its `scenarios/` to
/// `scenarios/`, and its recipe and shots to the project root. The project is then
/// opened for editing and seeded as `session` opens one (so the bundled scripts
/// are in it, as in every project), and the recipe runs on an empty scene in a
/// [`Session`] through the console's evaluator, called with the params table.
/// What the recipe raised fails the generation; the engine's warnings are printed
/// to stderr. Opening the project makes `out` the working directory.
pub fn generate(game: &Game, out: &Path, params: &Params) -> Result<(), String> {
    let out = &std::path::absolute(out).map_err(|e| format!("{}: {e}", out.display()))?;
    tree::clear(out)?;
    tree::copy_dir(
        &game.dir.join("scripts"),
        &out.join("assets").join("scripts"),
    )?;
    tree::copy_dir(&game.dir.join("scenarios"), &out.join("scenarios"))?;
    for file in [RECIPE, SHOTS] {
        let from = game.dir.join(file);
        if from.is_file() {
            std::fs::copy(&from, out.join(file)).map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }
    let marker = out.join(tree::MARKER);
    std::fs::write(&marker, format!("{}\n", game.name))
        .map_err(|e| format!("{}: {e}", marker.display()))?;

    let opened = crate::core::project::open(out, Access::Edit)?;
    crate::scene::print_refresh(&crate::scene::seed_project());
    let mut session = Session::boot(&BootPlan::default(), ProjectSource::bound(&opened))?;
    let line = format!("return assert(loadfile(\"{RECIPE}\"))({})", params.to_lua());
    let ran = session.command(&line);
    for warning in session.take_warnings() {
        eprintln!("[Game] {warning}");
    }
    ran.map(drop)
        .map_err(|e| format!("{}: {RECIPE}: {e}", game.name))
}
