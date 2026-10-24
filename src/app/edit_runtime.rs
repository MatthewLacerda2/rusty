//! src/app/edit_runtime.rs — the Lua runtime outside Play (#1339).
//!
//! Unity runs editor scripting outside Play, and so does rusty: the windowed
//! editor and the headless session both bring the evaluator up in edit mode, so
//! the console line and MCP `eval`/`snapshot` drive the authoritative edit scene.
//! Play replaces it with a fresh play runtime; Stop brings the edit one back.

use super::game::GameWorld;

impl GameWorld {
    /// Bring the Lua runtime live **in edit mode**, without entering Play.
    ///
    /// Play's `enter_play` initialises the runtime as a side effect of starting
    /// the simulation (snapshot, physics build, lifecycle `start`). An edit-mode
    /// host (the editor, the headless session) needs the *evaluator* live against
    /// the authoritative edit scene without any of that: no snapshot is taken, no
    /// entity scripts are loaded, and `physics` stays `None` (edit mode has no
    /// rapier world). The runtime binds the same shared `scene`/`input`/`nav`/…
    /// cells, so every `api::` namespace resolves against the live world and
    /// mutations stick. Stop restores it after Play
    /// ([`restore_edit_runtime`](Self::restore_edit_runtime)).
    ///
    /// Idempotent: a no-op once the runtime is live (re-initialising would drop
    /// any session state held in the VM). Errors surface as the Lua init message.
    pub fn init_edit_runtime(&mut self) -> Result<(), String> {
        self.resources.edit_runtime = true;
        if self.resources.script_manager.is_live() {
            return Ok(());
        }
        self.resources
            .script_manager
            .init_runtime(&self.resources.physics)
    }

    /// On Stop, after Play's runtime is shut down and the edit scene restored:
    /// bring the edit-mode runtime back when the host had one, so the console and
    /// `eval` keep working between Play sessions. A failure is logged, as Play's is.
    pub(super) fn restore_edit_runtime(&mut self) {
        if !self.resources.edit_runtime {
            return;
        }
        if let Err(err) = self.init_edit_runtime() {
            self.resources
                .console
                .borrow_mut()
                .error(format!("Lua init error: {err}"));
        }
    }
}

#[cfg(test)]
#[path = "edit_runtime_tests.rs"]
mod edit_runtime_tests;
