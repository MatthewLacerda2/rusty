use std::io::{self, BufReader};
use std::process::exit;

use rusty::dev::session::{self, Session};

const USAGE: &str = "usage: session [--project <dir>] [<scene>|--empty]
  (no scene)       boot the project's boot scene
  <scene>          boot this scene
  --empty          start from an empty scene
  --project <dir>  the game project (default ./project)
  -h, --help       print this and exit
Reads one Lua line per command on stdin, writes one JSON line per answer on stdout.";

fn main() {
    rusty::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);

    let (boot_scene, project) = session::boot_from_args(&args);

    let mut sess = match Session::boot(&boot_scene, project) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("session: failed to start: {}", e);
            exit(2);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(e) = session::run(&mut sess, BufReader::new(stdin.lock()), stdout.lock()) {
        eprintln!("session: channel error: {}", e);
        exit(2);
    }
}
