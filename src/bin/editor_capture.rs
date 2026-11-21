//! src/bin/editor_capture.rs — the whole editor to a PNG, headless (dev-only, #731).
//!
//! ```text
//! Usage:
//!   cargo run --bin editor-capture --features dev -- [options]
//!   make editor-capture OUT=before.png ARGS="--select Player"
//!
//! Options: `editor-capture --help` prints them (`USAGE`, below).
//! ```
//!
//! Deterministic: a fixed size, scale and frame count. Exits 0 when written, 1 when the
//! box has no GPU/software adapter (install lavapipe), 2 on bad arguments or errors.

use std::process::exit;

use rusty::app::ProjectSource;
use rusty::core::application::{GameViewSize, Gizmo};
use rusty::core::project::{Access, BootPlan};
use rusty::core::settings::video::parse_size;
use rusty::dev::session::Session;
use rusty::editor::ViewportTab;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};
use rusty::shell::editor::capture_picker;

#[path = "editor_capture/sample_picker.rs"]
mod sample_picker;

const USAGE: &str = "usage: editor-capture [options]
options (all optional):
  --project <dir>     the game project to open (default `./project`)
  --out <png>         where to write, from the launch directory
                      (default `editor-capture.png`)
  --scene <path>      a `.scene` file in the project (default: the built-in default)
  --select <name>     select this entity first (the Inspector shows its cards)
  --select-asset <p>  an asset file in the project to select first (its card)
  --frame             frame the selection in the Scene view (the F key)
  --inspect-reflection-probe <i>
                      inspect reflection probe i (0-based) first, as
                      `Editor.InspectReflectionProbe` does: its volume draws
  --play              draw the Play-mode chrome (the sim is not stepped)
  --game              show the Game tab instead of the Scene tab
  --size <W>x<H>      capture size (default 1600x900)
  --demo <file>       open this demo on the Game view's timeline
  --demo-tick <n>     replay the demo to tick n first (default 0)
  --pointer <X>,<Y>   hold the pointer here, in pixels (a hover label shows)
  --gizmo <Name>=on|off  draw a gizmo or not, as `Editor.SetGizmo` names it
                      (`UI`, `LightProbes`, `ReflectionProbes`, `Colliders`,
                      `NavMesh`; `GameGizmos` or `Stats` over `--game`);
                      repeatable
  --game-view <size>  the Game view's size, as its dropdown sets it: `Free`,
                      an aspect `W:H` or a fixed size `WxH`
  --preview [<mesh>]  open the selection's Inspector Preview tab (a material
                      card or an asset card) on this mesh: `Sphere` (default),
                      `Cube`, `Suzanne` or `Plane`; not with --picker
  --picker [<page>]   the project picker the editor starts on, over a
                      sample recent list; <page> is `new`, `open`,
                      `mismatch` (the different-engine prompt) or `focus`
                      (a focused row); without one, the project list
  -h, --help          print this and exit";

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let project = rusty::core::project::take_flag(&mut args).unwrap_or_else(|e| fail(&e));
    let (out, source, opts, page) = parse(&args).unwrap_or_else(|e| fail(&e));
    // `--out` is named from the launch directory; opening the project moves it.
    let out = std::path::absolute(&out).map_or(out, |p| p.to_string_lossy().into_owned());
    if let Some(page) = page {
        let mut picker = sample_picker::build(&page).unwrap_or_else(|e| fail(&e));
        let size = (opts.width, opts.height);
        let written = capture_picker(&mut picker, sample_picker::NOW, &out, size);
        return report(written, &out);
    }
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Run);
    let opened = opened.unwrap_or_else(|e| fail(&e));
    // Read-only: a capture draws the project as its settings stand and never
    // writes them (#1652). A demo then replays under its own header's settings.
    let project = ProjectSource::read_only(&opened);
    let written = match (source.demo, source.scene) {
        (Some(demo), _) => {
            let mut world = capture::default_world(project);
            let demo = std::path::Path::new(&demo);
            capture::capture_demo(&mut world, &out, &opts, demo, source.tick)
        }
        (None, Some(path)) => {
            let session = Session::boot(&BootPlan::of(&path), project);
            let session = session.unwrap_or_else(|e| fail(&e));
            capture::capture(session.world(), &out, &opts)
        }
        (None, None) => capture::capture(&capture::default_world(project), &out, &opts),
    };
    report(written, &out);
}

fn report(written: Result<bool, String>, out: &str) {
    match written {
        Ok(true) => println!("editor-capture: wrote {out} ({})", engine_note()),
        Ok(false) => {
            eprintln!("editor-capture: no GPU/software adapter (install mesa-vulkan-drivers)");
            exit(1);
        }
        Err(e) => fail(&e),
    }
}

/// Which engine drew it, as `results.json`'s `engine` says it: the commit, the
/// `engine/` content read, and whether it is the content this binary was built with.
fn engine_note() -> String {
    let stamp = rusty::core::project::engine_content::stamp();
    let found = stamp.content_found.as_deref().unwrap_or("unknown");
    let built = match stamp.content_matches {
        Some(true) => "as built",
        Some(false) => "NOT the content this binary was built with",
        None => "build content unknown",
    };
    format!("engine {}, content {found}, {built}", stamp.commit)
}

/// What the command line asked for: `(out, scene, options, picker page)`.
type Parsed = (String, Source, EditorCaptureOptions, Option<String>);

/// What the editor opens on: a scene file (or the built-in default), or a demo at
/// a tick.
#[derive(Default)]
struct Source {
    scene: Option<String>,
    demo: Option<String>,
    tick: u64,
}

fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut out = "editor-capture.png".to_string();
    let (mut source, mut opts, mut picker) =
        (Source::default(), EditorCaptureOptions::default(), None);
    let mut it = args.iter().peekable();
    while let Some(flag) = it.next() {
        if flag == "--picker" {
            let page = it.next_if(|v| sample_picker::PAGES.contains(&v.as_str()));
            picker = Some(page.map_or("projects", |v| v.as_str()).to_string());
            continue;
        }
        if flag == "--preview" {
            let mesh = it.next_if(|v| !v.starts_with("--"));
            opts.preview = Some(mesh.map_or(Ok(Default::default()), |m| capture::parse_mesh(m))?);
            continue;
        }
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--out" => out = value()?,
            "--scene" => source.scene = Some(value()?),
            "--demo" => {
                source.demo = Some(value()?);
                (opts.tab, opts.playing) = (ViewportTab::Game, true);
            }
            "--demo-tick" => source.tick = value()?.parse().map_err(|e| format!("{flag}: {e}"))?,
            "--pointer" => opts.pointer = Some(parse_point(&value()?)?),
            "--gizmo" => {
                let (gizmo, on) = parse_gizmo(&value()?)?;
                opts.gizmos.set(gizmo, on);
            }
            "--game-view" => {
                let text = value()?;
                opts.game_view.size = GameViewSize::parse(&text)
                    .ok_or(format!("--game-view wants Free, W:H or WxH, got {text:?}"))?;
            }
            "--select" => opts.select = Some(value()?),
            "--select-asset" => opts.select_asset = Some(value()?),
            "--frame" => opts.frame_selected = true,
            "--inspect-reflection-probe" => {
                let index = value()?.parse().map_err(|e| format!("{flag}: {e}"))?;
                opts.inspect_reflection_probe = Some(index);
            }
            "--play" => opts.playing = true,
            "--game" => opts.tab = ViewportTab::Game,
            "--size" => (opts.width, opts.height) = parse_size(&value()?)?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if picker.is_some() {
        capture::refuse_with_picker(opts.preview)?;
    }
    Ok((out, source, opts, picker))
}

/// `X,Y` in pixels.
fn parse_point(text: &str) -> Result<(f32, f32), String> {
    let bad = || format!("--pointer wants X,Y, got {text:?}");
    let (x, y) = text.split_once(',').ok_or_else(bad)?;
    let (x, y) = (x.trim().parse(), y.trim().parse());
    Ok((x.map_err(|_| bad())?, y.map_err(|_| bad())?))
}

/// `Name=on` or `Name=off`, `Name` a row of the gizmo table.
fn parse_gizmo(text: &str) -> Result<(Gizmo, bool), String> {
    let bad = || format!("--gizmo wants <Name>=on|off, got {text:?}");
    let (name, state) = text.split_once('=').ok_or_else(bad)?;
    let gizmo = Gizmo::named(name).ok_or_else(|| {
        let names: Vec<_> = Gizmo::ALL.iter().map(|g| g.row().name).collect();
        format!("--gizmo: no gizmo {name:?} (one of {})", names.join(", "))
    })?;
    match state {
        "on" => Ok((gizmo, true)),
        "off" => Ok((gizmo, false)),
        _ => Err(bad()),
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("editor-capture: {msg}");
    exit(2);
}
