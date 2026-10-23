//! Running Blender (#1072): asking its version, and running the export script.
//!
//! Blender runs headless with its factory settings, so a user's add-ons and
//! preferences never change what a conversion writes, and with scripts in the
//! `.blend` disabled: an import never runs code a downloaded file carries.

use crate::asset::ImportError;
use std::path::Path;
// lint: allow(subprocess) a `.blend` converts only through Blender (CLAUDE.md, *One format inside*); authoring only, never during Play
use std::process::{Command, Output, Stdio};

/// The export settings, pinned in one place (see the script's header).
const EXPORT: &str = include_str!("export.py");

/// How much of a failed run's output an error quotes, from its end.
const TAIL_LINES: usize = 12;

/// The version the Blender at `path` reports.
pub(super) fn version(path: &Path) -> Result<String, ImportError> {
    let output = run(path, Command::new(path).arg("--version"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    super::locate::parse_version(&text).ok_or_else(|| {
        ImportError::Parse(format!(
            "{} --version reported no Blender version",
            path.display()
        ))
    })
}

/// Export `source` with the Blender at `blender` into the folder `dir`.
pub(super) fn export(blender: &Path, source: &Path, dir: &Path) -> Result<(), ImportError> {
    let mut command = Command::new(blender);
    command
        .args(["--background", "--factory-startup", "--disable-autoexec"])
        .arg(source)
        .args(["--python-exit-code", "1", "--python-expr", EXPORT, "--"])
        .arg(dir);
    let output = run(blender, &mut command)?;
    if output.status.success() {
        return Ok(());
    }
    Err(ImportError::Parse(format!(
        "{}: Blender failed to export it ({}):\n{}",
        source.display(),
        output.status,
        tail(&output)
    )))
}

/// Run `command` (starting the program at `program`) to its end, output captured.
fn run(program: &Path, command: &mut Command) -> Result<Output, ImportError> {
    command
        .stdin(Stdio::null())
        .output()
        .map_err(|e| ImportError::Parse(format!("starting {}: {e}", program.display())))
}

/// The last lines of what a run printed, stderr after stdout.
fn tail(output: &Output) -> String {
    let text = [&output.stdout, &output.stderr]
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .join("\n");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}
