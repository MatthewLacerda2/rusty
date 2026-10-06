//! src/bin/editor_capture.rs — the whole editor to a PNG, headless (dev-only, #731).
//!
//! ```text
//! Usage:
//!   cargo run --bin editor-capture --features dev -- [options]
//!   make editor-capture OUT=before.png ARGS="--select Player"
//!
//! Options (all optional):
//!   --project <dir>     the game project to open (default `./project`, #829)
//!   --out <png>         where to write, from the launch directory
//!                       (default `editor-capture.png`)
//!   --scene <path>      a `.scene` file in the project (default: the built-in default)
//!   --select <name>     select this entity first (the Inspector shows its cards)
//!   --select-asset <p>  an asset file in the project to select first (its card)
//!   --frame             frame the selection in the Scene view (the F key)
//!   --play              draw the Play-mode chrome (the sim is not stepped)
//!   --game              show the Game tab instead of the Scene tab
//!   --size <W>x<H>      capture size (default 1600x900)
//!   --picker [<page>]   the project picker the editor starts on (#854), over a
//!                       sample recent list; <page> is `new`, `open` or
//!                       `mismatch` (the different-engine prompt)
//! ```
//!
//! Deterministic: a fixed size, scale and frame count. Exits 0 when written, 1 when the
//! box has no GPU/software adapter (install lavapipe), 2 on bad arguments or errors.

use std::process::exit;

use rusty::core::project::Access;
use rusty::dev::session::Session;
use rusty::editor::ViewportTab;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};
use rusty::shell::editor::capture_picker;

#[path = "editor_capture/sample_picker.rs"]
mod sample_picker;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = rusty::core::project::take_flag(&mut args).unwrap_or_else(|e| fail(&e));
    let (out, scene, opts, page) = parse(&args).unwrap_or_else(|e| fail(&e));
    // `--out` is named from the launch directory; opening the project moves it.
    let out = std::path::absolute(&out).map_or(out, |p| p.to_string_lossy().into_owned());
    if let Some(page) = page {
        let mut picker = sample_picker::build(&page).unwrap_or_else(|e| fail(&e));
        let size = (opts.width, opts.height);
        let written = capture_picker(&mut picker, sample_picker::NOW, &out, size);
        return report(written, &out);
    }
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Run);
    opened.unwrap_or_else(|e| fail(&e));
    let written = match scene {
        Some(path) => {
            let session = Session::new(&path).unwrap_or_else(|e| fail(&e));
            capture::capture(session.world(), &out, &opts)
        }
        None => capture::capture(&capture::default_world(), &out, &opts),
    };
    report(written, &out);
}

fn report(written: Result<bool, String>, out: &str) {
    match written {
        Ok(true) => println!("editor-capture: wrote {out}"),
        Ok(false) => {
            eprintln!("editor-capture: no GPU/software adapter (install mesa-vulkan-drivers)");
            exit(1);
        }
        Err(e) => fail(&e),
    }
}

/// What the command line asked for: `(out, scene, options, picker page)`.
type Parsed = (String, Option<String>, EditorCaptureOptions, Option<String>);

fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut out = "editor-capture.png".to_string();
    let (mut scene, mut opts, mut picker) = (None, EditorCaptureOptions::default(), None);
    let mut it = args.iter().peekable();
    while let Some(flag) = it.next() {
        if flag == "--picker" {
            let page = it.next_if(|v| sample_picker::PAGES.contains(&v.as_str()));
            picker = Some(page.map_or("projects", |v| v.as_str()).to_string());
            continue;
        }
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--out" => out = value()?,
            "--scene" => scene = Some(value()?),
            "--select" => opts.select = Some(value()?),
            "--select-asset" => opts.select_asset = Some(value()?),
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
    Ok((out, scene, opts, picker))
}

fn fail(msg: &str) -> ! {
    eprintln!("editor-capture: {msg}");
    exit(2);
}
