use super::*;

const OWN: &str = "bench-scenes";
const BIN: &str = "/repo/target/profiling/bench-scenes";

fn sample(secs: &str, frames: &[(&str, &str)]) -> String {
    let mut out = format!("  {secs}: \n");
    for (i, (sym, image)) in frames.iter().enumerate() {
        out.push_str(&format!("\t    55d0c0ffee{i:02x} {sym} ({image})\n"));
    }
    out + "\n"
}

fn win(start: u64, end: u64) -> Window {
    Window {
        start: start * 1_000_000_000,
        end: end * 1_000_000_000,
    }
}

#[test]
fn script_blocks_fold_root_first_with_unknown_frames_named_by_image() {
    let render = [
        ("rusty_render::Renderer::render", BIN),
        ("rusty_dev::bench::measure_posed", BIN),
        ("main", BIN),
    ];
    let unknown = [("[unknown]", "/usr/lib/libc.so.6"), ("main", BIN)];
    let text = [
        sample("10.5", &render),
        sample("10.6", &unknown),
        sample("10.7", &render),
    ]
    .concat();
    let (f, tally) = parse(&text, &[win(10, 11)], OWN);
    assert_eq!(f.samples(), 3);
    let stack = "main;rusty_dev::bench::measure_posed;rusty_render::Renderer::render";
    assert_eq!(f.0.get(stack), Some(&2));
    assert_eq!(f.0.get("main;[libc.so.6]"), Some(&1), "{:?}", f.0);
    assert_eq!((tally.samples, tally.own, tally.unknown), (3, 2, 1));
    assert_eq!(tally.images.get("bench-scenes"), Some(&2));
}

#[test]
fn only_the_samples_inside_a_window_are_kept() {
    let own = [("rusty_dev::bench::measure_posed", BIN)];
    let libc = [(
        "pthread_cond_timedwait",
        "/usr/lib/x86_64-linux-gnu/libc.so.6",
    )];
    // Stamps either side of the windows' edges: both edges count, so 5.0 and 10.0 are kept.
    let text = [
        sample("4.999999", &own),
        sample("5.000000", &own),
        sample("6.25", &libc),
        sample("7.500000001", &own),
        sample("9.0", &own),
        sample("10.000000", &libc),
        sample("12.0", &own),
    ]
    .concat();
    let (f, tally) = parse(&text, &[win(5, 7), win(9, 10)], OWN);
    assert_eq!(f.samples(), 4, "{:?}", f.0);
    assert_eq!(f.0.get("rusty_dev::bench::measure_posed"), Some(&2));
    assert_eq!(f.0.get("pthread_cond_timedwait"), Some(&2));
    assert_eq!((tally.own, tally.images.get("libc.so.6")), (2, Some(&2)));
}

#[test]
fn a_sample_without_a_call_stack_carries_its_frame_on_the_header() {
    let text = "  5.5:      55d0c0ffee10 rusty_dev::bench::measure_posed (/x/bench-scenes)\n\
                  5.6:      7f0000000010 [unknown] ([unknown])\n\
                  8.0:      55d0c0ffee10 main (/x/bench-scenes)\n";
    let (f, tally) = parse(text, &[win(5, 6)], OWN);
    assert_eq!(f.samples(), 2, "{:?}", f.0);
    assert_eq!((tally.own, tally.unknown), (1, 1));
}

#[test]
fn perfs_lost_sample_lines_become_warnings() {
    let said = "[ perf record: Woken up 3 times to write data ]\n\
                Warning:\nProcessed 9000 events and lost 12 chunks!\n\
                Processed 9000 events and lost 12 chunks!\n";
    assert_eq!(
        warnings(said),
        vec!["perf: Processed 9000 events and lost 12 chunks!".to_string()]
    );
}

#[test]
fn a_paranoid_refusal_says_what_to_set() {
    let said = refusal("Error: Access to performance monitoring is limited by perf_event_paranoid");
    assert!(
        said.contains("sudo sysctl kernel.perf_event_paranoid=2"),
        "{said}"
    );
}
