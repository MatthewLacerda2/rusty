//! Blender runs headless on factory settings with the `.blend`'s scripts disabled: an import
//! never runs code a downloaded file carries.

use super::locate::{RUSTY_BLENDER_TIMEOUT, VERSION_TIMEOUT};
use crate::ImportError;
use std::io::Read;
use std::path::Path;
// lint: allow(subprocess) a `.blend` converts only through Blender (CLAUDE.md, *One format inside*); authoring only, never during Play
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

const EXPORT: &str = include_str!("export.py");

const TAIL_LINES: usize = 12;

const POLL: Duration = Duration::from_millis(20);

const BUSY_TRIES: u32 = 100;
const BUSY_WAIT: Duration = Duration::from_millis(2);

/// `Err`: no answer (it could not start, or timed out), never remembered; `Ok(Err)`: an answer
/// naming no version.
pub(super) type Answer = Result<Result<String, ImportError>, ImportError>;

pub(super) fn version(path: &Path) -> Answer {
    version_within(path, VERSION_TIMEOUT)
}

pub(super) fn version_within(path: &Path, limit: Duration) -> Answer {
    let output = run(path, Command::new(path).arg("--version"), limit)
        .map_err(|e| e.error(&format!("{} --version", path.display()), ""))?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(super::locate::parse_version(&text).ok_or_else(|| {
        ImportError::Parse(format!(
            "{} --version reported no Blender version",
            path.display()
        ))
    }))
}

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

enum Failed {
    Start(ImportError),
    Timeout(Duration),
}

impl Failed {
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

fn run(program: &Path, command: &mut Command, limit: Duration) -> Result<Output, Failed> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn(command).map_err(|e| Failed::Start(start_error(program, &e)))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    match wait(&mut child, limit) {
        Ok(Some(status)) => Ok(Output {
            status,
            stdout: stdout.join().unwrap_or_default(),
            stderr: stderr.join().unwrap_or_default(),
        }),
        // Killed and reaped; the readers end with the pipes, since a process the program started
        // may still hold them.
        Ok(None) => match child.kill().and_then(|()| child.wait()) {
            Ok(_) => Err(Failed::Timeout(limit)),
            Err(e) => Err(Failed::Start(start_error(program, &e))),
        },
        Err(e) => Err(Failed::Start(start_error(program, &e))),
    }
}

/// Retries briefly on "text file busy" (`ETXTBSY`): a just-written program stays open for
/// writing in a process another thread is forking, until its `exec`.
fn spawn(command: &mut Command) -> std::io::Result<Child> {
    let mut tries = 1;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < BUSY_TRIES => {
                tries += 1;
                std::thread::sleep(BUSY_WAIT);
            }
            started => return started,
        }
    }
}

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

fn tail(output: &Output) -> String {
    let text = [&output.stdout, &output.stderr]
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .join("\n");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}
