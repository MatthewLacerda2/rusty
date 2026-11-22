use rusty::core::project::{self, Access};

const USAGE: &str = "usage: rusty [--project <dir>]
  (no option)      start on the project picker: recent projects, New, Open
  --project <dir>  open the game project at <dir> (created if missing)
  -h, --help       print this and exit";

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    project::answer_help(&args, USAGE);
    let named = project::take_flag(&mut args).unwrap_or_else(|e| usage(&e));
    if let Some(unknown) = args.first() {
        usage(&format!("unknown argument {unknown}"));
    }
    match named {
        Some(dir) => {
            let opened = project::open(&dir, Access::Edit).unwrap_or_else(|e| fail(&e));
            rusty::shell::editor::launch(&opened);
        }
        None => rusty::shell::editor::launch_picker(),
    }
}

/// A command line it cannot read: print `error` and the usage, exit 2, before any
/// window opens.
fn usage(error: &str) -> ! {
    eprintln!("rusty: {error}\n{USAGE}");
    std::process::exit(2);
}

fn fail(err: &str) -> ! {
    eprintln!("[Project] cannot open the project: {err}");
    std::process::exit(2);
}
