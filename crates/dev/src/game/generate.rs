use std::path::Path;

use super::{tree, Game, Params, RECIPE, SHARED, SHOTS};
use crate::session::Session;
use rusty_app::ProjectSource;
use rusty_core::project::{Access, BootPlan};

pub fn generate(game: &Game, out: &Path, params: &Params) -> Result<(), String> {
    let out = &std::path::absolute(out).map_err(|e| format!("{}: {e}", out.display()))?;
    tree::clear(out)?;
    tree::copy_dir(
        &game.dir.join("scripts"),
        &out.join("assets").join("scripts"),
    )?;
    if let Some(games) = game.dir.parent() {
        tree::copy_dir(
            &games.join(SHARED),
            &out.join("assets").join("scripts").join(SHARED),
        )?;
    }
    tree::copy_dir(&game.dir.join("scenarios"), &out.join("scenarios"))?;
    for file in [RECIPE, SHOTS] {
        let from = game.dir.join(file);
        if from.is_file() {
            std::fs::copy(&from, out.join(file)).map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }
    let marker = out.join(tree::MARKER);
    // lint: allow(asset-write) the generator's own marker, which no path-keyed cache reads
    std::fs::write(&marker, format!("{}\n", game.name))
        .map_err(|e| format!("{}: {e}", marker.display()))?;

    // Leaves `out` the working directory, so a caller holds its paths absolute.
    let opened = rusty_core::project::open(out, Access::Edit)?;
    rusty_edit::print_refresh(&rusty_edit::seed_project());
    let mut session = Session::boot(&BootPlan::default(), ProjectSource::bound(&opened))?;
    let line = format!("return assert(loadfile(\"{RECIPE}\"))({})", params.to_lua());
    let ran = session.command(&line);
    for warning in session.take_warnings() {
        eprintln!("[Game] {warning}");
    }
    ran.map(drop)
        .map_err(|e| format!("{}: {RECIPE}: {e}", game.name))
}
