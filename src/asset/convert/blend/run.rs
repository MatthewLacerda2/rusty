//! Running Blender (#1072): asking its version, and running the export script.
//!
//! Blender runs headless with its factory settings, so a user's add-ons and
//! preferences never change what a conversion writes, and with scripts in the
//! `.blend` disabled: an import never runs code a downloaded file carries.
//!
//! Every run has a time limit: one past it is killed and fails, so a hung Blender
//! never holds the editor, or the cache's lock, for good. The limit is counted in
//! sleeps between polls, not read off a clock (the sim modules read none).

use super::locate::{RUSTY_BLENDER_TIMEOUT, VERSION_TIMEOUT};
use crate::asset::ImportError;
use std::io::Read;
use std::path::Path;
// lint: allow(subprocess) a `.blend` converts only through Blender (CLAUDE.md, *One format inside*); authoring only, never during Play
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

/// The export settings, pinned in one place (see the script's header).
const EXPORT: &str = include_str!("export.py");

/// How much of a failed run's output an error quotes, from its end.
const TAIL_LINES: usize = 12;

/// How often a run is polled for its end.
const POLL: Duration = Duration::from_millis(20);

/// The version the Blender at `path` reports.
pub(super) fn version(path: &Path) -> Result<String, ImportError> {
    version_within(path, VERSION_TIMEOUT)
}

/// [`version`], asked within `limit`.
pub(super) fn version_within(path: &Path, limit: Duration) -> Result<String, ImportError> {
    let output = run(path, Command::new(path).arg("--version"), limit)
        .map_err(|e| e.error(&format!("{} --version", path.display()), ""))?;
    let text = String::from_utf8_lossy(&output.stdout);
    super::locate::parse_version(&text).ok_or_else(|| {
        ImportError::Parse(format!(
            "{} --version reported no Blender version",
            path.display()
        ))
    })
}

/// Export `source` with the Blender at `blender` into the folder `dir`, within
/// `timeout`.
pub(super) fn export(
    blender: &Path,
    source: &Path,
    dir: &Path,
    timeout: Duration,
) -> Result<(), ImportError> {
    let mut command = Command::new(blender);
    command
        .args(["--background", "--factory-startup", "--disable-autoexec"])
        .arg(source)
        .args(["--python-exit-code", "1", "--python-expr", EXPORT, "--"])
        .arg(dir);
    let output = run(blender, &mut command, timeout).map_err(|e| {
        let what = format!("{}: Blender exporting it", source.display());
        let hint = format!("; {RUSTY_BLENDER_TIMEOUT} sets a longer limit, in seconds");
        e.error(&what, &hint)
    })?;
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

/// Why a run gave no output.
enum Failed {
    /// The program could not be started.
    Start(ImportError),
    /// It ran past its limit, and was killed.
    Timeout(Duration),
}

impl Failed {
    /// The failure as an error about `what` was running; a timeout's says `hint`.
    fn error(self, what: &str, hint: &str) -> ImportError {
        match self {
            Self::Start(e) => e,
            Self::Timeout(limit) => ImportError::Parse(format!(
                "{what} did not finish within {} s, so it was stopped{hint}",
                limit.as_secs()
            )),
        }
    }
}

/// Run `command` (starting the program at `program`) to its end, output captured,
/// killing it once it has run `limit`.
fn run(program: &Path, command: &mut Command, limit: Duration) -> Result<Output, Failed> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failed::Start(start_error(program, &e)))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    match wait(&mut child, limit) {
        Ok(Some(status)) => Ok(Output {
            status,
            stdout: stdout.join().unwrap_or_default(),
            stderr: stderr.join().unwrap_or_default(),
        }),
        // Killed and reaped; the readers are left to end with the pipes, since a
        // process the program started may still hold them.
        Ok(None) => match child.kill().and_then(|()| child.wait()) {
            Ok(_) => Err(Failed::Timeout(limit)),
            Err(e) => Err(Failed::Start(start_error(program, &e))),
        },
        Err(e) => Err(Failed::Start(start_error(program, &e))),
    }
}

/// The run's status once it ends, `None` when it is still running after `limit`.
fn wait(child: &mut Child, limit: Duration) -> std::io::Result<Option<ExitStatus>> {
    let mut waited = Duration::ZERO;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if waited >= limit {
            return Ok(None);
        }
        std::thread::sleep(POLL);
        waited += POLL;
    }
}

/// Read all of `pipe` on a thread of its own, so neither pipe fills and stalls
/// the program while the other is read.
fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            // What was read before a failed read is kept: it is only quoted.
            if pipe.read_to_end(&mut bytes).is_err() {}
        }
        bytes
    })
}

fn start_error(program: &Path, e: &std::io::Error) -> ImportError {
    ImportError::Parse(format!("starting {}: {e}", program.display()))
}

/// The last lines of what a run printed, stderr after stdout.
fn tail(output: &Output) -> String {
    let text = [&output.stdout, &output.stderr]
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .join("\n");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}
