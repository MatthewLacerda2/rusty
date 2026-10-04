//! src/bin/editor_capture.rs — the whole editor to a PNG, headless (dev-only, #731).
//!
//! ```text
//! Usage:
//!   cargo run --bin editor-capture --features dev -- [options]
//!   make editor-capture OUT=before.png ARGS="--select Player"
//!
//! Options (all optional):
//!   --out <png>         where to write (default `editor-capture.png`)
//!   --scene <path>      a `.scene` file to open (default: the built-in default scene)
//!   --select <name>     select this entity first (the Inspector shows its cards)
//!   --frame             frame the selection in the Scene view (the F key)
//!   --play              draw the Play-mode chrome (the sim is not stepped)
//!   --game              show the Game tab instead of the Scene tab
//!   --size <W>x<H>      capture size (default 1600x900)
//! ```
//!
//! Deterministic: a fixed size, scale and frame count. Exits 0 when written, 1 when the
//! box has no GPU/software adapter (install lavapipe), 2 on bad arguments or errors.

use std::process::exit;

use rusty::dev::session::Session;
use rusty::editor::ViewportTab;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (out, scene, opts) = parse(&args).unwrap_or_else(|e| fail(&e));
    let written = match scene {
        Some(path) => {
            let session = Session::new(&path).unwrap_or_else(|e| fail(&e));
            capture::capture(session.world(), &out, &opts)
        }
        None => capture::capture(&capture::default_world(), &out, &opts),
    };
    match written {
        Ok(true) => println!("editor-capture: wrote {out}"),
        Ok(false) => {
            eprintln!("editor-capture: no GPU/software adapter (install mesa-vulkan-drivers)");
            exit(1);
        }
        Err(e) => fail(&e),
    }
}

/// `(out, scene, options)` from the command line.
fn parse(args: &[String]) -> Result<(String, Option<String>, EditorCaptureOptions), String> {
    let mut out = "editor-capture.png".to_string();
    let (mut scene, mut opts) = (None, EditorCaptureOptions::default());
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--out" => out = value()?,
            "--scene" => scene = Some(value()?),
            "--select" => opts.select = Some(value()?),
            "--frame" => opts.frame_selected = true,
            "--play" => opts.playing = true,
            "--game" => opts.tab = ViewportTab::Game,
            "--size" => {
                let size = value()?;
                let (w, h) = size.split_once('x').ok_or(format!("bad --size {size}"))?;
                let num = |s: &str| s.parse().map_err(|_| format!("bad --size {size}"));
                (opts.width, opts.height) = (num(w)?, num(h)?);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok((out, scene, opts))
}

fn fail(msg: &str) -> ! {
    eprintln!("editor-capture: {msg}");
    exit(2);
}
