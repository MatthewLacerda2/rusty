//! The example runner's own cases: what it must flag and what it must pass.

use super::check;
use super::fence::{lua_blocks, Mode};

fn only(doc: &str) -> super::fence::Block {
    let mut blocks = lua_blocks(doc).expect("parses");
    assert_eq!(blocks.len(), 1, "one block in {doc:?}");
    blocks.remove(0)
}

#[test]
fn reads_markers_indented_fences_and_skips_other_fences() {
    let doc = "```json\n```lua\n```\n\n  ```lua no-run with=hud:Canvas+Image,gun\n  a()\n    b()\n  ```\n";
    let blocks = lua_blocks(doc).expect("parses");
    assert_eq!(
        blocks.len(),
        1,
        "the ```lua inside the json fence is not a block"
    );
    let b = &blocks[0];
    assert_eq!((b.line, &b.mode), (5, &Mode::NoRun));
    assert_eq!(b.source, "a()\n  b()\n");
    let with: Vec<_> = b.with.iter().map(|(n, c)| (n.as_str(), c.len())).collect();
    assert_eq!(with, [("hud", 2), ("gun", 0)]);
    assert!(
        lua_blocks("```lua norun\n```\n").is_err(),
        "a typo'd marker is refused"
    );
}

#[test]
fn flags_a_call_to_a_missing_function() {
    let err = check(&only("```lua\nTransform.NoSuchThing(1)\n```\n")).unwrap_err();
    assert!(err.contains("NoSuchThing"), "{err}");
}

#[test]
fn flags_a_no_run_block_that_does_not_compile() {
    assert!(check(&only("```lua no-run\nlocal = 1\n```\n")).is_err());
    assert!(check(&only("```lua ignore\nlocal = 1\n```\n")).is_ok());
}

#[test]
fn passes_a_block_whose_entities_come_from_with() {
    let doc = "```lua with=crate:Light\nTransform.SetPosition(crate, 1, 2, 3)\nLight.SetIntensity(crate, 4)\n```\n";
    check(&only(doc)).expect("crate exists and carries a Light");
    let bare = "```lua\nLight.SetIntensity(crate, 4)\n```\n";
    assert!(check(&only(bare)).is_err(), "without with=, crate is nil");
}
