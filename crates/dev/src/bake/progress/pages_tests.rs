use std::cell::RefCell;
use std::time::Duration;

use super::{BakeLog, Line, Progress, Step};

#[test]
fn every_encoded_page_gets_a_line_within_the_interval() {
    let lines: RefCell<Vec<Line>> = RefCell::default();
    let mut log = BakeLog::new(|| Duration::ZERO, |l| lines.borrow_mut().push(l));
    let at = |done| Progress {
        step: Step::LightmapPages,
        done,
        total: 3,
        meshes: None,
    };
    for done in [0, 0, 1, 1, 2, 3] {
        log.observe(at(done));
    }
    drop(log);
    let texts: Vec<String> = lines.into_inner().into_iter().map(|l| l.text).collect();
    assert_eq!(
        texts,
        [
            "[Lighting] 0:00 lightmap pages 0% (0/3 pages encoded)",
            "[Lighting] 0:00 lightmap pages 33% (1/3 pages encoded), ~0 s left",
            "[Lighting] 0:00 lightmap pages 66% (2/3 pages encoded), ~0 s left",
            "[Lighting] 0:00 lightmap pages 100% (3/3 pages encoded)",
        ]
    );
}
