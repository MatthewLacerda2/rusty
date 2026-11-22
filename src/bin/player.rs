const USAGE: &str = "usage: player [--project <dir>]
  --project <dir>  the game project to run (default: the one shipped beside the
                   executable, else ./project)
  -h, --help       print this and exit";

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let opened = rusty::core::project::open_from_args(&mut args, rusty::core::project::Access::Run);
    rusty::shell::player::launch(opened.settings);
}
