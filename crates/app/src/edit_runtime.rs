use super::game::GameWorld;

impl GameWorld {
    /// Idempotent: re-initialising would drop session state held in the VM.
    pub fn init_edit_runtime(&mut self) -> Result<(), String> {
        self.resources.edit_runtime = true;
        if self.resources.script_manager.is_live() {
            return Ok(());
        }
        self.resources.script_manager.init_runtime()
    }

    pub(super) fn restore_edit_runtime(&mut self) {
        if !self.resources.edit_runtime {
            return;
        }
        let init = self.init_edit_runtime();
        self.report_lua_init(init);
    }

    /// Logs `Lua init error: …`; returns whether the runtime started.
    pub fn report_lua_init(&self, init: Result<(), String>) -> bool {
        let Err(err) = init else {
            return true;
        };
        self.resources
            .console
            .borrow_mut()
            .error(format!("Lua init error: {err}"));
        false
    }
}

#[cfg(test)]
#[path = "edit_runtime_tests.rs"]
mod edit_runtime_tests;
