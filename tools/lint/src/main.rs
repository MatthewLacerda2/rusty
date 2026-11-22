mod agent_docs;
mod code;
mod comments;
mod components;
mod dead_pub;
mod determinism;
mod gates;
mod layers;
mod modules;
mod parity;
mod repo;
mod rules;
mod size;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let check = gates::select(&args).unwrap_or_else(|why| {
        eprintln!("{why}");
        std::process::exit(2);
    });

    if args.iter().any(|a| a == gates::DEAD_PUB) {
        dead_pub::run();
        return;
    }
    if let Some(at) = args.iter().position(|a| a == comments::proof::FLAG) {
        let rev = args
            .get(at + 1)
            .map_or(comments::proof::DEFAULT_BASE, String::as_str);
        gates::report(&comments::proof::verdict(std::path::Path::new(""), rev));
        return;
    }
    if let Some(at) = args.iter().position(|a| a == comments::signals::FLAG) {
        print!(
            "{}",
            comments::signals::run(std::path::Path::new(""), &args[at + 1..])
        );
        return;
    }
    gates::report(&check(std::path::Path::new(""), &args));
}
