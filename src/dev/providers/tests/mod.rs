//! The provider boundary's rules, each pinned by a test (#383). No test here makes a
//! network call: nothing in the module can.

mod budget;
mod generated;
mod sfx;
mod speech;
mod voices;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

use super::*;

const KEY: &str = "sk_test_not_a_real_key";

/// A fresh, empty scratch folder per test, so parallel tests never share a ledger.
fn scratch(tag: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join(format!("rusty_383_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn keyed() -> Environment {
    Environment::of([(Provider::ElevenLabs.variable(), KEY)])
}

#[test]
fn a_key_resolves_from_the_environment() {
    let key = resolve(Provider::ElevenLabs, &keyed()).unwrap();
    assert_eq!(key.expose(), KEY);
    assert!(
        !format!("{key:?}").contains(KEY),
        "a Debug print must not leak the key"
    );
}

#[test]
fn a_missing_key_names_the_variable_and_where_it_looked() {
    let refusal = resolve(Provider::ElevenLabs, &Environment::default()).unwrap_err();
    let Refusal::MissingKey { variable, .. } = &refusal else {
        panic!("expected MissingKey, got {refusal:?}");
    };
    assert_eq!(*variable, "ELEVENLABS_API_KEY");
    assert!(refusal.to_string().contains("ELEVENLABS_API_KEY"));
    assert!(refusal.to_string().contains(".env"));
}

#[test]
fn the_dotenv_fills_gaps_and_never_overrides_the_environment() {
    let dir = scratch("dotenv");
    let only_in_file = "RUSTY_383_ONLY_IN_DOTENV";
    std::fs::write(
        dir.join(".env"),
        format!("PATH=from-file\n{only_in_file}=filled\n"),
    )
    .unwrap();
    let environment = Environment::discover(&dir);
    assert_eq!(environment.dotenv(), Some(dir.join(".env").as_path()));
    assert_eq!(environment.get(only_in_file), Some("filled"));
    assert_ne!(
        environment.get("PATH"),
        Some("from-file"),
        "the process env wins"
    );
}

#[test]
fn a_call_during_play_is_refused() {
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -1.0, 1.0, -1.0, 1.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.set_playing(true);
    let ledger = scratch("play").join("provider_budget.json");
    let refusal = permit(
        game.is_playing(),
        Provider::ElevenLabs,
        1,
        &keyed(),
        &ledger,
    );
    assert_eq!(refusal.unwrap_err(), Refusal::Playing);
    assert!(Refusal::Playing.to_string().contains("Play"));

    game.set_playing(false);
    assert!(permit(
        game.is_playing(),
        Provider::ElevenLabs,
        1,
        &keyed(),
        &ledger
    )
    .is_ok());
}

#[test]
fn play_is_checked_before_the_key() {
    let ledger = scratch("order").join("provider_budget.json");
    let refusal = permit(
        true,
        Provider::ElevenLabs,
        1,
        &Environment::default(),
        &ledger,
    );
    assert_eq!(refusal.unwrap_err(), Refusal::Playing);
}

#[test]
fn the_ledger_lives_at_the_project_root() {
    assert_eq!(ledger::path(), PathBuf::from("provider_budget.json"));
}
