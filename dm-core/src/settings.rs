use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub skip_splash: bool,
    pub progress_display: ProgressDisplay,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProgressDisplay {
    #[default]
    Bar,
    Text,
}

impl Settings {
    pub const COUNT: usize = 2;

    pub fn toggle(&mut self, index: usize) -> Result<()> {
        match index {
            0 => self.skip_splash = !self.skip_splash,
            1 => {
                self.progress_display = match self.progress_display {
                    ProgressDisplay::Bar => ProgressDisplay::Text,
                    ProgressDisplay::Text => ProgressDisplay::Bar,
                };
            }
            _ => return Ok(()),
        }
        self.save()
    }

    pub fn load() -> Result<Self> {
        let Some(path) = settings_path() else {
            return Ok(Self::default());
        };
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error).context("failed to read settings file"),
        };

        toml::from_str(&contents).context("failed to parse settings file")
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = settings_path() else {
            anyhow::bail!("could not determine the settings directory");
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create settings directory")?;
        }
        let contents = toml::to_string_pretty(self).context("failed to serialize settings")?;
        fs::write(path, contents).context("failed to write settings file")
    }
}

fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "Flux", "Flux").map(|dirs| dirs.config_dir().join("config.toml"))
}
