//! What a progressive bake shows between passes (#1970): the whole scene every
//! pass, converging on the finished bake.

use super::bake::progressive::progressive_on;
use super::progressive_tests::lit;
use super::tests::settings;
use super::*;

#[test]
fn every_pass_shows_the_whole_scene_converging() {
    let scene = BakeScene {
        lights: vec![],
        ..lit()
    };
    let s = BakeSettings {
        samples: 64,
        samples_per_pass: 16,
        ..settings()
    };
    let mut shown: Vec<Vec<Lightmap>> = Vec::new();
    let mut heard = |_: Pass, maps: Vec<Lightmap>| shown.push(maps);
    let progress = BakeProgress::default();
    let last = progressive_on(&scene, &s, &progress, (None, 2), &mut heard).expect("finished");
    let ended = Pass {
        done: 4,
        passes: 4,
        samples: 64,
        view: 64,
        target: 64,
    };
    assert_eq!(progress.passes(), Some(ended));
    let error = |maps: &[Lightmap]| {
        let pairs = maps
            .iter()
            .zip(&last)
            .flat_map(|(a, b)| a.texels.iter().zip(&b.texels));
        pairs
            .map(|(a, b)| (*a - *b).abs().element_sum())
            .sum::<f32>()
    };
    assert_eq!(shown.len(), 3);
    for maps in &shown {
        assert_eq!(
            maps.iter().map(|m| (m.entity, m.size)).collect::<Vec<_>>(),
            [(1, 8), (2, 8)]
        );
        assert!(maps
            .iter()
            .all(|m| m.texels.iter().all(|t| t.is_finite() && t.x > 0.0)));
    }
    let (first, third) = (error(&shown[0]), error(&shown[2]));
    assert!(
        third < first,
        "the shown lightmaps converge on the last: {first} then {third}"
    );
}
