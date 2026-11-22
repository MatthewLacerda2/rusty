//! The API verbs a scenario's VM makes in the gameplay VM, where the live scripts whose
//! callbacks they fire (`OnTakeDamage`) run. Recording (`bridge::forward`) and replay
//! (`calls::invoke`) both read this table, so a verb replays where it was made.

use mlua::Result as LuaResult;

pub struct Forward {
    pub namespace: &'static str,
    pub verbs: &'static [&'static str],
    /// Runs in the caller's VM while the game's is down: `Move` moves the edit-mode
    /// controller, but a `Damage` hit no handler can hear is the scenario's mistake.
    pub falls_back: bool,
}

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

pub fn find(namespace: &str, verb: &str) -> Option<&'static Forward> {
    FORWARDS
        .iter()
        .find(|f| f.namespace == namespace && f.verbs.contains(&verb))
}

impl Forward {
    /// `in_game` answers `None` while the gameplay VM is down; it is a closure so the
    /// caller's borrow of the world ends before `own` runs.
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
