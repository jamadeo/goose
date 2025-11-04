use std::{env, ffi::OsString, path::PathBuf};

use crate::config::{Config, ConfigError};

pub struct SearchPaths {
    paths: Vec<PathBuf>,
}

impl SearchPaths {
    pub fn builder() -> Self {
        let mut paths = Config::global()
            .get_goose_search_paths()
            .unwrap_or_default();

        #[cfg(unix)]
        {
            paths.push("/usr/local/bin".into());
            paths.push("~/.local/bin".into());
        }

        if cfg!(target_os = "macos") {
            paths.push("/opt/homebrew/bin".into());
        }

        Self {
            paths: paths
                .into_iter()
                .map(|s| PathBuf::from(shellexpand::tilde(&s).as_ref()))
                .collect(),
        }
    }

    pub fn with_npm(mut self) -> Self {
        if cfg!(windows) {
            if let Some(appdata) = dirs::data_dir() {
                self.paths.push(appdata.join("npm"));
            }
        } else if let Some(home) = dirs::home_dir() {
            self.paths.push(home.join(".npm-global/bin"));
        }
        self
    }

    pub fn env_var(self) -> Result<OsString, ConfigError> {
        env::join_paths(
            self.paths.into_iter().chain(
                env::var_os("PATH")
                    .as_ref()
                    .map(env::split_paths)
                    .into_iter()
                    .flatten(),
            ),
        )
        .map_err(|e| ConfigError::DeserializeError(format!("{e}")))
    }
}
