//! The API verbs a scenario's VM makes in the gameplay VM, and the one rule for
//! making them (#1879, #1893, #1929).
//!
//! Each verb here calls script callbacks synchronously, inside the call
//! (`OnTakeDamage`, `OnControllerColliderHit`), on the live script instances,
//! and those live in the gameplay VM: a caller's own copy of the namespace would
//! find none. Two callers make them: the scenario's VM while a demo records
//! (`bridge::forward`), and the replay's VM (`calls::invoke`). Both go through
//! [`Forward::run`], so a down runtime (before the first `Harness.Step`) is
//! handled the same way on both sides, and a replay repeats what was recorded.
//!
//! The table lives here, not in `bridge`, so `demo` reads it without importing
//! `bridge` (which imports `demo`).

use mlua::Result as LuaResult;

/// One namespace's forwarded verbs.
pub struct Forward {
    pub namespace: &'static str,
    pub verbs: &'static [&'static str],
    /// Whether a verb runs in the caller's own VM while the gameplay VM is
    /// down. `Move` does (it moves the edit-mode controller, and there is no
    /// handler yet to hear it); `Damage.*` refuses: a hit no handler can hear is
    /// a scenario's mistake.
    pub falls_back: bool,
}

/// Every forwarded verb, by namespace.
pub const FORWARDS: [Forward; 2] = [
    Forward {
        namespace: "Damage",
        verbs: &["Apply", "ApplyPoint", "ApplyRadial"],
        falls_back: false,
    },
    Forward {
        namespace: "CharacterController",
        verbs: &["Move"],
        falls_back: true,
    },
];

/// The entry forwarding `namespace.verb`, if one does.
pub fn find(namespace: &str, verb: &str) -> Option<&'static Forward> {
    FORWARDS
        .iter()
        .find(|f| f.namespace == namespace && f.verbs.contains(&verb))
}

impl Forward {
    /// Make `verb`: `in_game` tries it in the gameplay VM (`None` while that VM
    /// is down, as `ScriptManager::with_vm` answers); then `own` makes it in the
    /// caller's VM if this namespace falls back, and it is an error otherwise.
    /// `in_game` is a closure so a caller's borrow of the world ends before
    /// `own` runs.
    pub fn run<R>(
        &self,
        verb: &str,
        in_game: impl FnOnce() -> LuaResult<Option<R>>,
        own: impl FnOnce() -> LuaResult<R>,
    ) -> LuaResult<R> {
        match in_game()? {
            Some(returns) => Ok(returns),
            None if self.falls_back => own(),
            None => Err(mlua::Error::RuntimeError(format!(
                "{}.{verb}: the game's runtime is down",
                self.namespace
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down() -> LuaResult<Option<u8>> {
        Ok(None)
    }

    #[test]
    fn a_down_runtime_falls_back_only_where_the_table_says() {
        let moves = find("CharacterController", "Move").unwrap();
        assert_eq!(moves.run("Move", down, || Ok(7)).unwrap(), 7);
        let damage = find("Damage", "Apply").unwrap();
        let err = damage.run("Apply", down, || Ok(7)).unwrap_err();
        assert!(err
            .to_string()
            .contains("Damage.Apply: the game's runtime is down"));
        let up = damage.run("Apply", || Ok(Some(1)), || Ok(7)).unwrap();
        assert_eq!(up, 1, "an up runtime takes the call");
        assert!(find("Transform", "SetPosition").is_none());
    }
}
