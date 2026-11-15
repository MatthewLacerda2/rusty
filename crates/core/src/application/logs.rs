//! The project's rolling frame log settings on the `Application` (#2206): read with
//! the rest of `project.rusty`, switched by `Debug.SetLogChannels` and the Build
//! Settings window, and written back so the choice outlives the session.

use super::Application;
use crate::settings::logs::LogSettings;

impl Application {
    /// The frame log's settings as last read or set: the frame loop asks every
    /// frame, so this reads no file.
    pub fn log_settings(&self) -> LogSettings {
        self.logs
    }

    /// Switch the named channels (`Debug.SetLogChannels`, the Build Settings
    /// window), then write the bound file (if any) once. An unknown name is an
    /// error, and nothing changes.
    pub fn set_log_channels(&mut self, switches: &[(String, bool)]) -> Result<(), String> {
        self.reread();
        let mut channels = self.logs.channels;
        for (name, on) in switches {
            channels.set(name, *on)?;
        }
        self.logs.channels = channels;
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use crate::application::Application;
    use crate::project::ProjectFile;

    #[test]
    fn a_channel_switch_is_kept_in_the_project_file() {
        let dir = rusty_test_support::temp::dir();
        let path = dir.join("project.rusty");
        let mut app = Application::new();
        app.open(&path).unwrap();
        let off = [("audio".to_string(), false)];
        app.set_log_channels(&off).unwrap();
        assert!(!app.log_settings().channels.audio);
        let file = ProjectFile::read(&path).unwrap().unwrap();
        assert!(!file.logs.channels.audio && file.logs.channels.frames);
        let bad = [("frames".to_string(), false), ("nope".to_string(), true)];
        assert!(app.set_log_channels(&bad).is_err());
        assert!(
            app.log_settings().channels.frames,
            "a refused switch changes nothing"
        );
    }
}
